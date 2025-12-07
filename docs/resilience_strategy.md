# Resilience & Failure-Handling Strategy

This document describes how the current implementation handles timeouts, retries, circuit breakers, agent health, and graceful shutdown in the master orchestrator and agents. It reflects the behavior implemented in the Rust code today and does not describe unimplemented resource-limiting features.

---

## 1. Timeouts & Retries

### 1.1 AgentExecutionConfig

Per-agent execution behavior is configured via shared types in `core/shared_types/src/lib.rs`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentRetryConfig {
    pub max_attempts: u8,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentCircuitBreakerConfig {
    pub failure_threshold: u32,
    pub cooldown_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentExecutionConfig {
    pub timeout_secs: u64,
    pub retry: AgentRetryConfig,
    pub circuit_breaker: AgentCircuitBreakerConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentsConfig {
    pub default: AgentExecutionConfig,
    pub git_agent: Option<AgentExecutionConfig>,
    pub obsidian_agent: Option<AgentExecutionConfig>,
    pub llm_router_agent: Option<AgentExecutionConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub llm: LLMConfig,
    #[serde(default)]
    pub agents: Option<AgentsConfig>,
}
```

Semantics:

- `timeout_secs` – per-invocation wall-clock timeout for an agent process.
- `retry` – retry policy for transient failures:
  - `max_attempts` – maximum number of attempts (initial call + retries).
  - `initial_backoff_ms` – base delay for exponential backoff.
  - `max_backoff_ms` – upper bound on any single backoff delay.
- `circuit_breaker` – thresholds used by the health/circuit-breaker logic:
  - `failure_threshold` – consecutive failure count required to open the circuit.
  - `cooldown_ms` – duration to keep the circuit open before re-allowing calls.

If `app_config.agents` is `None`, the executor uses built-in defaults in `core/master_orchestrator/src/executor.rs`:

```rust
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
```

### 1.2 Per-Agent Timeouts

All agent invocations go through `execute_agent`:

```rust
pub async fn execute_agent(
    agent_name: &str,
    request: &ActionRequest,
    timeout_duration: Duration,
) -> Result<ActionResponse, ToolError> {
    // spawn child ...

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

    // status checking and JSON parsing ...
}
```

`timeout_duration` is derived from `AgentExecutionConfig.timeout_secs` for the selected agent, or falls back to the default 30 seconds when no `agents` configuration is provided.

If the agent process does not complete within the configured timeout:

- The call returns `ToolError::Timeout`.
- The executor treats this as a failure that may be retried depending on the retry policy and error classification.

### 1.3 Retry Policy & Backoff

Retries are implemented in `execute_agent_with_retries`:

```rust
struct AgentRetryPolicy {
    max_attempts: u8,
    initial_backoff_ms: u64,
    max_backoff_ms: u64,
}

async fn execute_agent_with_retries( /* ... */ ) -> Result<ActionResponse, ToolError> {
    // set initial task states Dispatched and InProgress ...

    let mut attempt: u8 = 1;
    loop {
        let result = execute_agent(agent_name, request, timeout_duration).await;

        match result {
            Ok(resp) => {
                if resp.status == "success" && resp.code == 0 {
                    // mark Succeeded, update health, return Ok(resp)
                } else {
                    // handle non-success response
                }
            }
            Err(e) => {
                // handle ToolError case
            }
        }
    }
}
```

Exponential backoff is computed by `compute_backoff_ms`:

```rust
fn compute_backoff_ms(policy: &AgentRetryPolicy, attempt: u8) -> u64 {
    let exp = if attempt == 0 { 0 } else { (attempt - 1) as u32 };

    let factor = match 1u64.checked_shl(exp) {
        Some(v) => v,
        None => u64::MAX,
    };

    let base = policy.initial_backoff_ms.saturating_mul(factor);
    base.min(policy.max_backoff_ms)
}
```

Effective behavior:

- For attempts `1, 2, 3, ...` the unconstrained backoff sequence is:

  ```
  backoff_ms = initial_backoff_ms * (2 ^ (attempt - 1))
  ```

- Each backoff is clamped to `max_backoff_ms`.
- Example with `initial_backoff_ms = 500`, `max_backoff_ms = 5_000`:

  - `attempt = 1` → 500 ms
  - `attempt = 2` → 1_000 ms
  - `attempt = 3` → 2_000 ms
  - `attempt = 4` → 4_000 ms
  - `attempt = 5` → 5_000 ms (capped)

Each time a retry is taken, the executor:

1. Records `TaskStatus::Retried` with the latest `AgentError`.
2. Sleeps for the computed backoff.
3. Increments `attempt` and calls the agent again.

When retries are exhausted or the error is non-retryable, the task is dead-lettered (see below).

### 1.4 Task Lifecycle Transitions

The v1 entrypoint in `plan_and_execute_v1` creates a plan and a single root task and records their lifecycle:

1. Plan creation & initial states:

   - `PlanStatus::Draft`
   - (on readiness) `PlanStatus::Pending`
   - Just before executing, `PlanStatus::Running`

2. Root task creation:

   - `TaskStatus::Queued`

Within `execute_agent_with_retries`, the task moves through the following states:

1. `TaskStatus::Dispatched` – the executor has accepted the task for execution.
2. `TaskStatus::InProgress` – agent process is actively being invoked.
3. Zero or more `TaskStatus::Retried` transitions – for each retryable failure.
4. Terminal state:
   - `TaskStatus::Succeeded` – when the agent returns `status == "success"` and `code == 0`.
   - `TaskStatus::DeadLettered` – when retries are exhausted or the error is classified as non-retryable.

These state changes are recorded via `MemoryService::record_task_state_change`, which currently logs to stdout. Plan-level changes use `MemoryService::record_plan_state_change` and similarly log-only today.

### 1.5 Error Classification & Retry Decisions

Error classification is centralized in `map_agent_failure_to_agent_error`:

```rust
fn map_agent_failure_to_agent_error(
    agent_name: &str,
    resp: Option<&ActionResponse>,
    tool_err: Option<&ToolError>,
) -> AgentError { /* ... */ }
```

Two families of failure are handled:

#### Non-Success Agent Responses

When an agent returns an `ActionResponse` but with non-success status or code, the executor treats `code` like an HTTP status:

```rust
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
```

- 4xx → `AgentErrorCode::InvalidRequest`
- 501 → `AgentErrorCode::ActionNotSupported`
- 504 → `AgentErrorCode::Timeout`
- Anything else → `AgentErrorCode::BackendFailure`

#### Tool-Level Failures

When the orchestrator fails to spawn or communicate with the agent process, `ToolError` values are mapped to `AgentErrorCode`:

```rust
match err {
    ToolError::Timeout(msg) => (AgentErrorCode::Timeout, msg.clone()),
    ToolError::IOError(msg) => (AgentErrorCode::Io, msg.clone()),
    ToolError::SerializationError(msg)
    | ToolError::DeserializationError(msg)
    | ToolError::InvalidAgentResponse(msg) => (AgentErrorCode::BackendFailure, msg.clone()),
    ToolError::ExecutionError(msg) => (AgentErrorCode::BackendFailure, msg.clone()),
}
```

#### Retry vs Permanent Failure

Whether a failure is retried is controlled by both the retry budget and the error code:

```rust
let should_retry = attempt < retry_policy.max_attempts
    && matches!(
        agent_err.code,
        AgentErrorCode::BackendFailure
            | AgentErrorCode::Timeout
            | AgentErrorCode::Io
            | AgentErrorCode::Internal
    );
```

- **Retryable** codes:
  - `BackendFailure` (generic downstream/server error)
  - `Timeout` (agent or orchestrator timeout)
  - `Io` (transient IO issues)
  - `Internal` (reserved; currently not emitted by the mapper)

- **Non-retryable** codes:
  - `InvalidRequest` (caller or payload error)
  - `ActionNotSupported` (capability gap)

On final failure after retries or a non-retryable error:

- The task is marked `TaskStatus::DeadLettered`.
- Agent health is updated via `update_agent_health_on_failure`.
- For process-level failures (`ToolError`), the executor returns `Err(ToolError)`; for agent-level non-success responses, the executor returns `Ok(ActionResponse)` with a non-zero code.

---

## 2. Circuit Breakers & Agent Health

### 2.1 Shared Types

Agent health states and summaries are modeled in shared types:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentHealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentHealthSummaryV1 {
    pub agent_id: AgentId,
    pub health: AgentHealthState,
    pub consecutive_failures: u32,
    pub last_failure_at: Option<String>,
    pub last_success_at: Option<String>,
    pub circuit_open_until: Option<String>,
}
```

These are returned by the `/api/v1/agents` endpoint.

### 2.2 Persistence Model

Agent health and circuit state are stored in the `agent_health` table created by `MemoryService::init_gai_memory`:

```sql
CREATE TABLE IF NOT EXISTS agent_health (
    tool_name TEXT PRIMARY KEY,
    health TEXT NOT NULL,
    consecutive_failures INTEGER NOT NULL,
    last_failure_at TEXT,
    last_success_at TEXT,
    circuit_open_until TEXT
)
```

The string `health` column is mapped back to `AgentHealthState` by `map_health_str`.

`MemoryService` provides:

- `update_agent_health_on_success(tool_name, now_iso, breaker_cfg)` – marks the agent healthy.
- `update_agent_health_on_failure(tool_name, now_iso, breaker_cfg)` – increments failure counts, applies circuit logic, and returns the updated `AgentHealthSummaryV1`.
- `get_agent_health(tool_name)` – returns the current summary, or a default healthy summary if no record exists.
- `list_agent_health()` – returns summaries for all agents.

### 2.3 Success Path

On a successful agent call (`status == "success"` and `code == 0`), the executor calls:

```rust
let now_iso = chrono::Utc::now().to_rfc3339();
let _ = memory_service
    .update_agent_health_on_success(agent_name, &now_iso)
    .await;
```

`update_agent_health_on_success`:

- Sets `health = "healthy"`.
- Resets `consecutive_failures = 0`.
- Sets `last_success_at = now_iso`.
- Clears `last_failure_at` and `circuit_open_until`.

### 2.4 Failure Path & Circuit Breaker

On final failure (after retries or for non-retryable error codes), the executor calls:

```rust
let now_iso = chrono::Utc::now().to_rfc3339();
let _ = memory_service
    .update_agent_health_on_failure(agent_name, &now_iso, breaker_cfg)
    .await;
```

Key logic in `update_agent_health_on_failure`:

1. Load existing `consecutive_failures` (defaults to 0 if no row).
2. Increment to `new_failures`.
3. Decide the new health string and possibly a circuit deadline:

   ```rust
   let (health_str, circuit_open_until): (String, Option<String>) =
       if new_failures >= breaker.failure_threshold {
           let deadline = (chrono::Utc::now()
               + chrono::Duration::milliseconds(breaker.cooldown_ms as i64))
           .to_rfc3339();
           ("unhealthy".to_string(), Some(deadline))
       } else {
           ("degraded".to_string(), None)
       };
   ```

4. Upsert the row in `agent_health` and read it back as `AgentHealthSummaryV1`.

Effective behavior:

- Before `failure_threshold` is reached:
  - `health` transitions to `"degraded"`.
  - `consecutive_failures` increments.
  - `circuit_open_until` remains `NULL`.
- Once `consecutive_failures >= failure_threshold`:
  - `health` is set to `"unhealthy"`.
  - `circuit_open_until` is set to `now + cooldown_ms`.

### 2.5 Circuit Breaker in Planning (Short-Circuiting Unhealthy Agents)

The v1 planner consults agent health before executing:

```rust
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
```

Behavior:

- If an agent is currently **unhealthy** *and* the `circuit_open_until` timestamp is in the future:
  - The plan is immediately marked `PlanStatus::Failed`.
  - The planner returns an error with `OrchestratorErrorCode::AgentUnavailable`.
  - No agent process is spawned for this request.
- If the circuit has expired (or was never opened), the planner proceeds and lets the executor attempt a call, giving the agent a chance to recover.

### 2.6 `/api/v1/agents` Endpoint

The `/api/v1/agents` endpoint in the HTTP API surfaces agent health:

```rust
async fn list_agents(
   req: HttpRequest,
   ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
   if let Err(resp) = require_auth(&req, ctx.get_ref()) {
       return Ok(resp);
   }

   let summaries = ctx
       .memory_service
       .list_agent_health()
       .await
       .map_err(actix_web::error::ErrorInternalServerError)?;

   Ok(HttpResponse::Ok().json(summaries))
}
```

This returns a JSON array of `AgentHealthSummaryV1`, enabling dashboards and operational tooling to monitor:

- Current health state (`healthy`, `degraded`, `unhealthy`).
- Failure counts and recent timestamps.
- Circuit breaker state via `circuit_open_until`.

---

## 3. Graceful Shutdown

The orchestrator uses Actix Web’s built-in graceful shutdown support plus a placeholder memory shutdown hook.

### 3.1 Shutdown Signals

In `main.rs`, after constructing the HTTP server, the code sets up a CTRL+C handler:

```rust
let server = run_http_server(api_ctx, BIND_ADDRESS)?;
let handle = server.handle();

let shutdown_fut = async move {
    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("[WARN] Failed to install CTRL+C handler: {}", e);
        return;
    }
    println!("[INFO] Received CTRL+C, initiating graceful shutdown...");
    handle.stop(true).await;
};

tokio::select! {
    res = server => {
        if let Err(e) = res {
            eprintln!("[ERROR] HTTP server error: {}", e);
        }
    }
    _ = shutdown_fut => {
        println!("[INFO] Shutdown signal handled.");
    }
}
```

Key aspects:

- `tokio::signal::ctrl_c()` waits for a SIGINT/CTRL+C.
- On signal, `server.handle().stop(true).await` is called.

### 3.2 Actix Server Behavior

`handle.stop(true)` triggers Actix Web’s graceful shutdown:

- The server stops accepting new incoming connections.
- In-flight requests are allowed to complete, up to Actix’s internal shutdown timeout.
- The future returned by `HttpServer::run()` resolves once all workers have shut down.

The orchestrator does **not** currently implement per-request cancellation or explicit draining beyond Actix’s default behavior.

### 3.3 MemoryService Shutdown Hook

After the HTTP server shuts down, the orchestrator calls a placeholder shutdown hook:

```rust
// Placeholder for future flush logic; currently a no-op.
memory_service.shutdown().await;
```

`MemoryService::shutdown` is currently implemented as:

```rust
pub async fn shutdown(&self) {
    // Currently a no-op; reserved for graceful shutdown flushing.
}
```

There is no explicit flush or close behavior today; this function exists solely as a future extension point for:

- Flushing in-memory buffers to persistent storage.
- Closing Sled or SQLite connections in a controlled manner.

---

At present, **no OS-level CPU/memory limits or cgroup/job-object controls are implemented**. Resilience is provided through:

- Configurable per-agent timeouts and retries.
- Error classification to avoid retrying invalid or unsupported operations.
- Per-agent circuit breakers based on consecutive failures and cooldown windows.
- Plan-level short-circuiting of unhealthy agents.
- Basic graceful shutdown for the HTTP server and a reserved hook for memory shutdown.