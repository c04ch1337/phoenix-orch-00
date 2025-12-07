use crate::memory_service::MemoryService;
use jsonschema::JSONSchema;
use once_cell::sync::Lazy;
use platform::{correlation_span, record_counter, record_histogram};
use serde_json::{self, json, Value};
use shared_types::{
    ActionRequest, ActionResponse, AgentCircuitBreakerConfig, AgentError, AgentErrorCode,
    AppConfig, CorrelationId, PlanId, TaskId, TaskStatus, ToolError,
};
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};

fn build_action_response_schema() -> JSONSchema {
    let schema_json = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {
            "request_id": { "type": "string" },
            "api_version": { "type": ["string", "null"] },
            "status": { "type": "string" },
            "code": {
                "type": "integer",
                "minimum": 0,
                "maximum": 65535
            },
            "result": {
                "type": ["object", "null"],
                "properties": {
                    "output_type": { "type": "string" },
                    "data": { "type": "string" },
                    "metadata": {}
                },
                "required": ["output_type", "data"],
                "additionalProperties": false
            },
            "error": {
                "type": ["string", "null"]
            },
            "plan_id": { "type": ["string", "null"] },
            "task_id": { "type": ["string", "null"] },
            "correlation_id": { "type": ["string", "null"] }
        },
        "required": ["request_id", "status", "code"],
        "additionalProperties": false
    });

    JSONSchema::compile(&schema_json).expect("ActionResponse JSON Schema must be valid")
}

fn validate_and_parse_action_response(raw: &str) -> Result<ActionResponse, ToolError> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|e| ToolError::InvalidAgentResponse(format!("Invalid JSON from agent: {e}")))?;

    let schema = build_action_response_schema();

    if let Err(errors) = schema.validate(&value) {
        let details = errors
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join("; ");
        return Err(ToolError::InvalidAgentResponse(format!(
            "Agent response failed schema validation: {details}"
        )));
    }

    serde_json::from_value(value).map_err(|e| {
        ToolError::InvalidAgentResponse(format!(
            "Failed to deserialize ActionResponse after schema validation: {e}"
        ))
    })
}

struct AgentRetryPolicy {
    max_attempts: u8,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

static AGENT_CONCURRENCY: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));

fn map_agent_failure_to_agent_error(
    agent_name: &str,
    resp: Option<&ActionResponse>,
    tool_err: Option<&ToolError>,
) -> AgentError {
    if let Some(r) = resp {
        // Non-success response from agent; interpret code as HTTP-like.
        let code = r.code;
        let message = r
            .error
            .clone()
            .unwrap_or_else(|| format!("agent {} reported error (code={})", agent_name, code));
        let agent_code = if (400..500).contains(&code) {
            AgentErrorCode::InvalidRequest
        } else if code == 501 {
            AgentErrorCode::ActionNotSupported
        } else if code == 504 {
            AgentErrorCode::Timeout
        } else {
            AgentErrorCode::BackendFailure
        };

        return AgentError {
            code: agent_code,
            message,
            details: None,
        };
    }

    // Tool-level error (spawn, IO, timeout, serialization, etc).
    let err = tool_err.expect("tool_err must be provided when resp is None");
    let (agent_code, message) = match err {
        ToolError::Timeout(msg) => (AgentErrorCode::Timeout, msg.clone()),
        ToolError::IOError(msg) => (AgentErrorCode::Io, msg.clone()),
        ToolError::SerializationError(msg)
        | ToolError::DeserializationError(msg)
        | ToolError::InvalidAgentResponse(msg) => (AgentErrorCode::BackendFailure, msg.clone()),
        ToolError::ExecutionError(msg) => (AgentErrorCode::BackendFailure, msg.clone()),
    };

    AgentError {
        code: agent_code,
        message,
        details: None,
    }
}

pub async fn execute_agent(
    agent_name: &str,
    request: &ActionRequest,
    timeout_duration: Duration,
) -> Result<ActionResponse, ToolError> {
    // Assuming binaries are in target/debug for development
    // In a real scenario, this path would be configurable
    let binary_name = if cfg!(target_os = "windows") {
        format!("{}.exe", agent_name)
    } else {
        agent_name.to_string()
    };

    let binary_path = std::env::current_dir()
        .map_err(|e| ToolError::IOError(e.to_string()))?
        .join("target/debug")
        .join(&binary_name);

    let request_json = serde_json::to_string(request).map_err(|e| {
        ToolError::SerializationError(format!("Failed to serialize request: {}", e))
    })?;

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            ToolError::IOError(format!(
                "Failed to spawn agent {} at {:?}: {}",
                agent_name, binary_path, e
            ))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| ToolError::IOError(format!("Failed to write to stdin: {}", e)))?;
    }

    let output_result = timeout(timeout_duration, child.wait_with_output()).await;

    let output = match output_result {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(ToolError::IOError(format!(
                "Failed to wait on child: {}",
                e
            )))
        }
        Err(_) => {
            return Err(ToolError::Timeout(format!(
                "Agent {} timed out after {} seconds",
                agent_name,
                timeout_duration.as_secs()
            )))
        }
    };

    if !output.status.success() {
        return Err(ToolError::ExecutionError(format!(
            "Agent exited with non-zero status: {:?}",
            output.status
        )));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| ToolError::InvalidAgentResponse(format!("Invalid UTF-8 from agent: {e}")))?;

    let response = validate_and_parse_action_response(&stdout)?;

    Ok(response)
}

async fn execute_agent_with_retries(
    agent_name: &str,
    plan_id: PlanId,
    task_id: TaskId,
    correlation_id: CorrelationId,
    request: &ActionRequest,
    memory_service: &MemoryService,
    retry_policy: AgentRetryPolicy,
    timeout_duration: Duration,
    breaker_cfg: &AgentCircuitBreakerConfig,
) -> Result<ActionResponse, ToolError> {
    // Initial lifecycle states for this task.
    memory_service
        .record_task_state_change(
            task_id,
            plan_id,
            TaskStatus::Dispatched,
            None,
            correlation_id,
        )
        .await
        .map_err(ToolError::ExecutionError)?;

    // Metrics: record task start at first dispatch.
    record_counter("orchestrator_task_started_total", 1);
    let task_start = Instant::now();

    memory_service
        .record_task_state_change(
            task_id,
            plan_id,
            TaskStatus::InProgress,
            None,
            correlation_id,
        )
        .await
        .map_err(ToolError::ExecutionError)?;

    let mut attempt: u8 = 1;
    loop {
        // Global concurrency limit for in-flight agent executions, and per-call
        // latency/failed-call metrics.
        let result = {
            let _permit = AGENT_CONCURRENCY
                .acquire()
                .await
                .expect("agent concurrency semaphore closed");

            let agent_start = Instant::now();
            let result = execute_agent(agent_name, request, timeout_duration).await;
            let duration = agent_start.elapsed().as_secs_f64();
            record_histogram("agent_call_duration_seconds", duration);
            if result.is_err() {
                record_counter("agent_call_failures_total", 1);
            }
            result
        };

        match result {
            Ok(resp) => {
                if resp.status == "success" && resp.code == 0 {
                    // Mark task as succeeded.
                    memory_service
                        .record_task_state_change(
                            task_id,
                            plan_id,
                            TaskStatus::Succeeded,
                            None,
                            correlation_id,
                        )
                        .await
                        .map_err(ToolError::ExecutionError)?;

                    // Record total task duration on terminal success.
                    record_histogram(
                        "orchestrator_task_duration_seconds",
                        task_start.elapsed().as_secs_f64(),
                    );

                    // Update agent health on final success.
                    let now_iso = chrono::Utc::now().to_rfc3339();
                    let _ = memory_service
                        .update_agent_health_on_success(agent_name, &now_iso)
                        .await;

                    return Ok(resp);
                }

                // Non-success response from agent.
                let agent_err = map_agent_failure_to_agent_error(agent_name, Some(&resp), None);

                let should_retry = attempt < retry_policy.max_attempts
                    && matches!(
                        agent_err.code,
                        AgentErrorCode::BackendFailure
                            | AgentErrorCode::Timeout
                            | AgentErrorCode::Io
                            | AgentErrorCode::Internal
                    );

                if should_retry {
                    memory_service
                        .record_task_state_change(
                            task_id,
                            plan_id,
                            TaskStatus::Retried,
                            Some(agent_err.clone()),
                            correlation_id,
                        )
                        .await
                        .map_err(ToolError::ExecutionError)?;

                    let backoff_ms = compute_backoff_ms(&retry_policy, attempt);
                    sleep(Duration::from_millis(backoff_ms)).await;
                    attempt += 1;
                    continue;
                } else {
                    // Final failure; dead-letter the task.
                    memory_service
                        .record_task_state_change(
                            task_id,
                            plan_id,
                            TaskStatus::DeadLettered,
                            Some(agent_err.clone()),
                            correlation_id,
                        )
                        .await
                        .map_err(ToolError::ExecutionError)?;

                    // Record total task duration on terminal failure.
                    record_histogram(
                        "orchestrator_task_duration_seconds",
                        task_start.elapsed().as_secs_f64(),
                    );

                    // Update agent health on final failure.
                    let now_iso = chrono::Utc::now().to_rfc3339();
                    let _ = memory_service
                        .update_agent_health_on_failure(agent_name, &now_iso, breaker_cfg)
                        .await;

                    return Ok(resp);
                }
            }
            Err(e) => {
                let agent_err = map_agent_failure_to_agent_error(agent_name, None, Some(&e));

                let should_retry = attempt < retry_policy.max_attempts
                    && matches!(
                        agent_err.code,
                        AgentErrorCode::BackendFailure
                            | AgentErrorCode::Timeout
                            | AgentErrorCode::Io
                            | AgentErrorCode::Internal
                    );

                if should_retry {
                    memory_service
                        .record_task_state_change(
                            task_id,
                            plan_id,
                            TaskStatus::Retried,
                            Some(agent_err.clone()),
                            correlation_id,
                        )
                        .await
                        .map_err(ToolError::ExecutionError)?;

                    let backoff_ms = compute_backoff_ms(&retry_policy, attempt);
                    sleep(Duration::from_millis(backoff_ms)).await;
                    attempt += 1;
                    continue;
                } else {
                    // Final failure; dead-letter the task and return error.
                    let _ = memory_service
                        .record_task_state_change(
                            task_id,
                            plan_id,
                            TaskStatus::DeadLettered,
                            Some(agent_err),
                            correlation_id,
                        )
                        .await;

                    // Record total task duration on terminal failure.
                    record_histogram(
                        "orchestrator_task_duration_seconds",
                        task_start.elapsed().as_secs_f64(),
                    );

                    // Update agent health on final failure.
                    let now_iso = chrono::Utc::now().to_rfc3339();
                    let _ = memory_service
                        .update_agent_health_on_failure(agent_name, &now_iso, breaker_cfg)
                        .await;

                    return Err(e);
                }
            }
        }
    }
}

fn compute_backoff_ms(policy: &AgentRetryPolicy, attempt: u8) -> u64 {
    let exp = if attempt == 0 {
        0
    } else {
        (attempt - 1) as u32
    };

    // Compute 2^exp as a u64 using checked_shl to avoid overflow panics.
    let factor = match 1u64.checked_shl(exp) {
        Some(v) => v,
        None => u64::MAX,
    };

    let base = policy.initial_backoff_ms.saturating_mul(factor);
    base.min(policy.max_backoff_ms)
}

pub async fn execute_agent_for_task(
    agent_name: &str,
    plan_id: PlanId,
    task_id: TaskId,
    correlation_id: CorrelationId,
    request: &mut ActionRequest,
    memory_service: &MemoryService,
    app_config: &AppConfig,
) -> Result<ActionResponse, ToolError> {
    let span = correlation_span(correlation_id, "execute_agent_for_task");
    let _enter = span.enter();
    tracing::info!(
        agent = %agent_name,
        plan_id = %plan_id,
        task_id = %task_id,
        correlation_id = %correlation_id,
        "executing agent for task"
    );

    // Ensure identifiers are set on the request before dispatch.
    request.plan_id = Some(plan_id);
    request.task_id = Some(task_id);
    request.correlation_id = Some(correlation_id);

    // Resolve per-agent execution configuration.
    let (timeout_duration, retry_policy, breaker_cfg) = if let Some(agents_cfg) = &app_config.agents
    {
        let exec_cfg = match agent_name {
            "git_agent" => agents_cfg.git_agent.as_ref().unwrap_or(&agents_cfg.default),
            "obsidian_agent" => agents_cfg
                .obsidian_agent
                .as_ref()
                .unwrap_or(&agents_cfg.default),
            "llm_router_agent" => agents_cfg
                .llm_router_agent
                .as_ref()
                .unwrap_or(&agents_cfg.default),
            _ => &agents_cfg.default,
        };

        let retry_policy = AgentRetryPolicy {
            max_attempts: exec_cfg.retry.max_attempts,
            initial_backoff_ms: exec_cfg.retry.initial_backoff_ms,
            max_backoff_ms: exec_cfg.retry.max_backoff_ms,
        };
        let timeout_duration = Duration::from_secs(exec_cfg.timeout_secs);
        let breaker_cfg = exec_cfg.circuit_breaker.clone();
        (timeout_duration, retry_policy, breaker_cfg)
    } else {
        // Reasonable built-in defaults if agents config is not provided.
        let retry_policy = AgentRetryPolicy {
            max_attempts: 3,
            initial_backoff_ms: 500,
            max_backoff_ms: 5_000,
        };
        let timeout_duration = Duration::from_secs(30);
        let breaker_cfg = AgentCircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_ms: 60_000,
        };
        (timeout_duration, retry_policy, breaker_cfg)
    };

    execute_agent_with_retries(
        agent_name,
        plan_id,
        task_id,
        correlation_id,
        request,
        memory_service,
        retry_policy,
        timeout_duration,
        &breaker_cfg,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::{ActionResult, ApiVersion};
    use uuid::Uuid;

    #[test]
    fn compute_backoff_ms_grows_exponentially_and_is_capped() {
        let policy = AgentRetryPolicy {
            max_attempts: 5,
            initial_backoff_ms: 500,
            max_backoff_ms: 5_000,
        };

        // attempt = 1 -> 500ms
        assert_eq!(compute_backoff_ms(&policy, 1), 500);
        // attempt = 2 -> 1000ms
        assert_eq!(compute_backoff_ms(&policy, 2), 1_000);
        // attempt = 3 -> 2000ms
        assert_eq!(compute_backoff_ms(&policy, 3), 2_000);
        // attempt = 4 -> 4000ms
        assert_eq!(compute_backoff_ms(&policy, 4), 4_000);
        // attempt = 5 -> 8000ms but capped at 5000ms
        assert_eq!(compute_backoff_ms(&policy, 5), 5_000);
    }

    fn make_action_response_with_code(code: u16) -> ActionResponse {
        ActionResponse {
            request_id: Uuid::new_v4(),
            api_version: Some(ApiVersion::V1),
            status: "error".to_string(),
            code,
            result: Some(ActionResult {
                output_type: "text".to_string(),
                data: "".to_string(),
                metadata: None,
            }),
            error: Some(format!("error-{code}")),
            plan_id: None,
            task_id: None,
            correlation_id: None,
        }
    }

    #[test]
    fn map_agent_failure_to_agent_error_classifies_response_codes() {
        // 4xx -> InvalidRequest
        let resp_400 = make_action_response_with_code(400);
        let err_400 = map_agent_failure_to_agent_error("test_agent", Some(&resp_400), None);
        assert!(matches!(err_400.code, AgentErrorCode::InvalidRequest));

        // 501 -> ActionNotSupported
        let resp_501 = make_action_response_with_code(501);
        let err_501 = map_agent_failure_to_agent_error("test_agent", Some(&resp_501), None);
        assert!(matches!(err_501.code, AgentErrorCode::ActionNotSupported));

        // 504 -> Timeout
        let resp_504 = make_action_response_with_code(504);
        let err_504 = map_agent_failure_to_agent_error("test_agent", Some(&resp_504), None);
        assert!(matches!(err_504.code, AgentErrorCode::Timeout));

        // 500 -> BackendFailure (default case)
        let resp_500 = make_action_response_with_code(500);
        let err_500 = map_agent_failure_to_agent_error("test_agent", Some(&resp_500), None);
        assert!(matches!(err_500.code, AgentErrorCode::BackendFailure));
    }

    #[test]
    fn map_agent_failure_to_agent_error_classifies_tool_errors() {
        // Timeout -> Timeout
        let timeout_err = ToolError::Timeout("timed out".to_string());
        let err = map_agent_failure_to_agent_error("test_agent", None, Some(&timeout_err));
        assert!(matches!(err.code, AgentErrorCode::Timeout));

        // IOError -> Io
        let io_err = ToolError::IOError("disk error".to_string());
        let err = map_agent_failure_to_agent_error("test_agent", None, Some(&io_err));
        assert!(matches!(err.code, AgentErrorCode::Io));

        // Serialization / deserialization / invalid agent response -> BackendFailure
        let ser_err = ToolError::SerializationError("ser".to_string());
        let derr = map_agent_failure_to_agent_error("test_agent", None, Some(&ser_err));
        assert!(matches!(derr.code, AgentErrorCode::BackendFailure));

        let deser_err = ToolError::DeserializationError("de".to_string());
        let derr2 = map_agent_failure_to_agent_error("test_agent", None, Some(&deser_err));
        assert!(matches!(derr2.code, AgentErrorCode::BackendFailure));

        let invalid_err = ToolError::InvalidAgentResponse("bad".to_string());
        let derr3 = map_agent_failure_to_agent_error("test_agent", None, Some(&invalid_err));
        assert!(matches!(derr3.code, AgentErrorCode::BackendFailure));

        // ExecutionError -> BackendFailure
        let exec_err = ToolError::ExecutionError("exec".to_string());
        let derr4 = map_agent_failure_to_agent_error("test_agent", None, Some(&exec_err));
        assert!(matches!(derr4.code, AgentErrorCode::BackendFailure));
    }
}
