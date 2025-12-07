use crate::executor::{execute_agent, execute_agent_for_task};
use crate::memory_service::MemoryService;
use crate::tool_service::ToolService;
use platform::{correlation_span, record_counter};
use serde_json::json;
use shared_types::{
    ActionRequest, ActionResponse, ActionResult, AgentHealthState, AppConfig, CorrelationId,
    OrchestratorError, OrchestratorErrorCode, Payload, PlanId, PlanStatus, TaskId, TaskStatus,
};
use std::sync::Arc;
use uuid::Uuid;

/// Output contract for the v1 planning/execution entrypoint.
pub struct PlanAndExecuteOutputV1 {
    pub plan_id: PlanId,
    pub root_task_id: TaskId,
    pub user_facing_output: String,
}

/// Error contract for the v1 planning/execution entrypoint.
pub struct PlanAndExecuteErrorV1 {
    pub correlation_id: CorrelationId,
    pub plan_id: Option<PlanId>,
    pub error: OrchestratorError,
}

fn agent_in_circuit_cooldown(
    health: AgentHealthState,
    circuit_open_until: &Option<String>,
) -> bool {
    if health != AgentHealthState::Unhealthy {
        return false;
    }

    if let Some(until) = circuit_open_until {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) {
            return dt > chrono::Utc::now();
        }
    }

    false
}

// Planner's main job: turn user intent into an ActionRequest
// This is the legacy entrypoint used by /api/chat and remains for backward compatibility.
pub async fn plan_and_execute(
    user_message: String,
    memory_service: Arc<MemoryService>,
    app_config: Arc<AppConfig>,
    tool_service: Arc<ToolService>,
) -> Result<ActionResponse, String> {
    // 1. Check Agent Registry
    let active_agents = memory_service
        .get_active_agents()
        .await
        .map_err(|e| format!("Memory Error: {}", e))?;

    // 2. Simple Intent Detection (Keyword-based for V1)
    let target_tool = if user_message.to_lowercase().contains("git")
        || user_message.to_lowercase().contains("commit")
    {
        "git_agent"
    } else {
        "llm_router_agent" // Fallback to the future LLM planning agent
    };

    // 3. Agent Validation Check (The Gatekeeper)
    if !active_agents.iter().any(|a| a.tool_name == target_tool) {
        return Err(format!(
            "Error: Agent '{}' is not registered or active.",
            target_tool
        ));
    }

    // --- Context Retrieval ---
    let mut context_str = String::new();

    // A. Structured Retrieval (Simple keyword match for now)
    // In a real system, we'd extract entities. Here we just try the whole message or keywords.
    if let Ok(facts) = memory_service
        .retrieve_structured_context(&user_message)
        .await
    {
        if !facts.is_empty() {
            context_str.push_str("\n[Structured Memory]:\n");
            for fact in facts {
                context_str.push_str(&format!("- {}\n", fact));
            }
        }
    }

    // B. Semantic Retrieval
    if let Ok(memories) = memory_service
        .retrieve_semantic_context(&user_message, 3)
        .await
    {
        if !memories.is_empty() {
            context_str.push_str("\n[Semantic Memory]:\n");
            for mem in memories {
                context_str.push_str(&format!("- {}\n", mem));
            }
        }
    }

    // 4. Prepare Payload
    let mut payload_json = json!({ "prompt": user_message });

    // Inject LLM Config if target is llm_router_agent
    if target_tool == "llm_router_agent" {
        let default_provider = &app_config.llm.default_provider;
        let provider_config = match default_provider.as_str() {
            "openrouter" => &app_config.llm.openrouter,
            "gemini" => &app_config.llm.gemini,
            "grok" => &app_config.llm.grok,
            "openai" => &app_config.llm.openai,
            "anthropic" => &app_config.llm.anthropic,
            "ollama" => &app_config.llm.ollama,
            "lmstudio" => &app_config.llm.lmstudio,
            _ => &None,
        };

        if let Some(config) = provider_config {
            // Append context to the prompt for the LLM
            let final_prompt = if !context_str.is_empty() {
                format!("{}\n\nContext:\n{}", user_message, context_str)
            } else {
                user_message.clone()
            };

            payload_json = json!({
                "prompt": final_prompt,
                "config": {
                    "provider": default_provider,
                    "api_key": config.api_key,
                    "base_url": config.base_url,
                    "model_name": config.model_name
                }
            });
        }
    }

    // 5. Create the ActionRequest (The Universal Contract)
    let request = ActionRequest {
        request_id: Uuid::new_v4(),
        api_version: None,
        tool: target_tool.to_string(),
        action: "execute".to_string(), // Default action for now, agents can parse args
        context: context_str,          // Also pass context in the dedicated field
        plan_id: None,
        task_id: None,
        correlation_id: None,
        payload: Payload(payload_json),
    };

    // 6. Execute based on whether it's a tool or an agent
    let response = if tool_service.tools.contains_key(target_tool) {
        // It's a direct tool command
        let parts: Vec<&str> = user_message.split_whitespace().collect();
        let params = if parts.len() > 1 { &parts[1..] } else { &[] };

        let tool_result = tool_service.execute_tool(target_tool, params).await?;

        // Wrap the raw string output in an ActionResponse
        let result = ActionResult {
            output_type: "text".to_string(),
            data: tool_result,
            metadata: None,
        };

        ActionResponse {
            request_id: request.request_id,
            api_version: None,
            status: "success".to_string(), // Or parse from tool_result if it includes status
            code: 0,
            result: Some(result),
            error: None,
            plan_id: None,
            task_id: None,
            correlation_id: None,
        }
    } else {
        // It's an agent. Resolve a timeout from config (or fall back to a sane default)
        let timeout_duration = if let Some(agents_cfg) = &app_config.agents {
            let exec_cfg = match target_tool {
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
            std::time::Duration::from_secs(exec_cfg.timeout_secs)
        } else {
            // Legacy default when no agents config is provided.
            std::time::Duration::from_secs(30)
        };

        execute_agent(target_tool, &request, timeout_duration)
            .await
            .map_err(|e| e.to_string())?
    };

    // Log action trace (This will now also index the action semantically!)
    if let Err(e) = memory_service.log_action_trace(&request, &response).await {
        eprintln!("Failed to log action trace: {}", e);
    }

    Ok(response)
}

/// New v1 planning + execution entrypoint that wires in plan/task lifecycle tracking.
pub async fn plan_and_execute_v1(
    correlation_id: CorrelationId,
    user_message: String,
    context: Option<String>,
    memory_service: Arc<MemoryService>,
    app_config: Arc<AppConfig>,
    tool_service: Arc<ToolService>,
) -> Result<PlanAndExecuteOutputV1, PlanAndExecuteErrorV1> {
    let span = correlation_span(correlation_id, "plan_and_execute_v1");
    let _enter = span.enter();
    record_counter("orchestrator_plan_started_total", 1);

    let plan_id = PlanId::new_v4();

    // Initial draft state with the raw user message as description.
    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Draft,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    // 1. Check Agent Registry
    let active_agents =
        memory_service
            .get_active_agents()
            .await
            .map_err(|e| PlanAndExecuteErrorV1 {
                correlation_id,
                plan_id: Some(plan_id),
                error: OrchestratorError {
                    code: OrchestratorErrorCode::PlanningFailed,
                    message: format!("Memory Error: {}", e),
                    details: None,
                },
            })?;

    // 2. Simple Intent Detection (Keyword-based for V1)
    let target_tool = if user_message.to_lowercase().contains("git")
        || user_message.to_lowercase().contains("commit")
    {
        "git_agent"
    } else {
        "llm_router_agent" // Fallback to the future LLM planning agent
    };

    // 3. Agent Validation Check (The Gatekeeper)
    if !active_agents.iter().any(|a| a.tool_name == target_tool) {
        let msg = format!(
            "Error: Agent '{}' is not registered or active.",
            target_tool
        );
        // Mark plan as failed before returning.
        let _ = memory_service
            .record_plan_state_change(plan_id, PlanStatus::Failed, Some(&msg), correlation_id)
            .await;
        record_counter("orchestrator_plan_failed_total", 1);

        return Err(PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::PlanningFailed,
                message: msg,
                details: None,
            },
        });
    }

    // 3b. Agent health check (Circuit Breaker).
    //
    // If the selected agent is marked as Unhealthy AND the circuit breaker is
    // still open (circuit_open_until is in the future), fail the plan early
    // with AgentUnavailable so that callers get a fast, clear failure.
    if let Ok(summary) = memory_service.get_agent_health(target_tool).await {
        if summary.health == AgentHealthState::Unhealthy {
            let mut in_cooldown = false;
            if let Some(ref until) = summary.circuit_open_until {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) {
                    if dt > chrono::Utc::now() {
                        in_cooldown = true;
                    }
                }
            }

            if in_cooldown {
                let msg = format!(
                    "Agent '{}' is temporarily unavailable due to recent failures.",
                    target_tool
                );

                let _ = memory_service
                    .record_plan_state_change(
                        plan_id,
                        PlanStatus::Failed,
                        Some(&msg),
                        correlation_id,
                    )
                    .await;
                record_counter("orchestrator_plan_failed_total", 1);

                return Err(PlanAndExecuteErrorV1 {
                    correlation_id,
                    plan_id: Some(plan_id),
                    error: OrchestratorError {
                        code: OrchestratorErrorCode::AgentUnavailable,
                        message: msg,
                        details: None,
                    },
                });
            }
        }
    }

    // --- Context Retrieval ---
    let mut context_str = String::new();

    // Include explicit request context if provided.
    if let Some(ref ctx) = context {
        if !ctx.is_empty() {
            context_str.push_str("\n[Request Context]:\n");
            context_str.push_str(ctx);
            context_str.push('\n');
        }
    }

    // A. Structured Retrieval (Simple keyword match for now)
    if let Ok(facts) = memory_service
        .retrieve_structured_context(&user_message)
        .await
    {
        if !facts.is_empty() {
            context_str.push_str("\n[Structured Memory]:\n");
            for fact in facts {
                context_str.push_str(&format!("- {}\n", fact));
            }
        }
    }

    // B. Semantic Retrieval
    if let Ok(memories) = memory_service
        .retrieve_semantic_context(&user_message, 3)
        .await
    {
        if !memories.is_empty() {
            context_str.push_str("\n[Semantic Memory]:\n");
            for mem in memories {
                context_str.push_str(&format!("- {}\n", mem));
            }
        }
    }

    // 4. Prepare Payload
    let mut payload_json = json!({ "prompt": user_message });

    // Inject LLM Config if target is llm_router_agent
    if target_tool == "llm_router_agent" {
        let default_provider = &app_config.llm.default_provider;
        let provider_config = match default_provider.as_str() {
            "openrouter" => &app_config.llm.openrouter,
            "gemini" => &app_config.llm.gemini,
            "grok" => &app_config.llm.grok,
            "openai" => &app_config.llm.openai,
            "anthropic" => &app_config.llm.anthropic,
            "ollama" => &app_config.llm.ollama,
            "lmstudio" => &app_config.llm.lmstudio,
            _ => &None,
        };

        if let Some(config) = provider_config {
            // Append context to the prompt for the LLM
            let final_prompt = if !context_str.is_empty() {
                format!("{}\n\nContext:\n{}", user_message, context_str)
            } else {
                user_message.clone()
            };

            payload_json = json!({
                "prompt": final_prompt,
                "config": {
                    "provider": default_provider,
                    "api_key": config.api_key,
                    "base_url": config.base_url,
                    "model_name": config.model_name
                }
            });
        }
    }

    // 5. Create the root task and record lifecycle.
    let root_task_id = TaskId::new_v4();

    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Pending,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    memory_service
        .record_task_state_change(
            root_task_id,
            plan_id,
            TaskStatus::Queued,
            None,
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Running,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    // 6. Create the ActionRequest (The Universal Contract)
    let mut request = ActionRequest {
        request_id: Uuid::new_v4(),
        api_version: None,
        tool: target_tool.to_string(),
        action: "execute".to_string(), // Default action for now, agents can parse args
        context: context_str,          // Also pass context in the dedicated field
        plan_id: None,
        task_id: None,
        correlation_id: Some(correlation_id),
        payload: Payload(payload_json),
    };

    // 7. Execute via the lifecycle-aware executor wrapper.
    let exec_result = execute_agent_for_task(
        target_tool,
        plan_id,
        root_task_id,
        correlation_id,
        &mut request,
        memory_service.as_ref(),
        app_config.as_ref(),
    )
    .await;

    match exec_result {
        Ok(response) => {
            // Mark plan as succeeded.
            memory_service
                .record_plan_state_change(
                    plan_id,
                    PlanStatus::Succeeded,
                    Some(&user_message),
                    correlation_id,
                )
                .await
                .map_err(|e| PlanAndExecuteErrorV1 {
                    correlation_id,
                    plan_id: Some(plan_id),
                    error: OrchestratorError {
                        code: OrchestratorErrorCode::Internal,
                        message: e,
                        details: None,
                    },
                })?;
            record_counter("orchestrator_plan_succeeded_total", 1);

            // Log action trace (This will now also index the action semantically!)
            if let Err(e) = memory_service.log_action_trace(&request, &response).await {
                eprintln!("Failed to log action trace: {}", e);
            }

            let user_facing_output = serde_json::to_string(&response.result).unwrap_or_default();

            Ok(PlanAndExecuteOutputV1 {
                plan_id,
                root_task_id,
                user_facing_output,
            })
        }
        Err(e) => {
            // Mark plan as failed; ignore logging errors here.
            let _ = memory_service
                .record_plan_state_change(
                    plan_id,
                    PlanStatus::Failed,
                    Some(&format!("Execution failed: {}", e)),
                    correlation_id,
                )
                .await;

            record_counter("orchestrator_plan_failed_total", 1);

            Err(PlanAndExecuteErrorV1 {
                correlation_id,
                plan_id: Some(plan_id),
                error: OrchestratorError {
                    code: OrchestratorErrorCode::ExecutionFailed,
                    message: e.to_string(),
                    details: None,
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn agent_in_circuit_cooldown_returns_false_for_non_unhealthy_or_missing_deadline() {
        // Healthy agents should never be considered in cooldown.
        assert!(!agent_in_circuit_cooldown(AgentHealthState::Healthy, &None));

        // Unhealthy but without a circuit_open_until timestamp is treated as not in cooldown.
        assert!(!agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &None
        ));
    }

    #[test]
    fn agent_in_circuit_cooldown_checks_future_and_past_deadlines() {
        let future = (Utc::now() + Duration::minutes(5)).to_rfc3339();
        let past = (Utc::now() - Duration::minutes(5)).to_rfc3339();

        // Unhealthy with a future deadline -> in cooldown.
        assert!(agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &Some(future)
        ));

        // Unhealthy but deadline in the past -> no longer in cooldown.
        assert!(!agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &Some(past)
        ));
    }
}
