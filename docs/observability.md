# Observability

This document describes how to observe the orchestrator in production using logs, metrics, correlation IDs, and the agent-health API. It reflects the current implementation and does not assume unimplemented features.

---

## 1. Overview

The orchestrator exposes three primary observability surfaces:

1. **Logs**
   - Implemented with `tracing` via [`Rust.fn init_tracing()`](core/platform/src/tracing.rs:1) and initialized in [`Rust.fn main()`](core/master_orchestrator/src/main.rs:89).
   - Structured `tracing::info!` / `tracing::error!` events include fields such as `correlation_id`, `plan_id`, `task_id`, and `agent`.

2. **Metrics**
   - Implemented via the `metrics` crate and `metrics-exporter-prometheus`.
   - Exported over HTTP using [`Rust.fn init_metrics()`](core/platform/src/metrics.rs:18), which binds a `/metrics` endpoint on the configured `METRICS_ADDR`.
   - Recording helpers:
     - [`Rust.fn record_counter()`](core/platform/src/metrics.rs:35)
     - [`Rust.fn record_histogram()`](core/platform/src/metrics.rs:40)

3. **Correlation IDs**
   - `ChatRequestV1` accepts an optional `correlation_id`, defaulting to a new UUID when omitted, as defined in [`Rust.struct ChatRequestV1`](core/shared_types/src/lib.rs:307).
   - The planner and executor wrap work in `tracing` spans created via [`Rust.fn correlation_span()`](core/platform/src/tracing.rs:1); these spans carry the `correlation_id` consistently through logs and metrics.

---

## 2. Key Metrics

The following metrics are emitted by the orchestrator and tools. All names are simple string constants; there are no labels at this time.

### 2.1 Planning

Emitted from [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221):

- `orchestrator_plan_started_total` (counter)
  - Incremented once at the beginning of each v1 chat/plan request.
- `orchestrator_plan_succeeded_total` (counter)
  - Incremented when a plan completes successfully.
- `orchestrator_plan_failed_total` (counter)
  - Incremented when a plan fails due to validation, planning, agent unavailability, or execution errors.

### 2.2 Task and Agent Execution

Emitted from [`Rust.fn execute_agent_with_retries()`](core/master_orchestrator/src/executor.rs:218):

- `orchestrator_task_started_total` (counter)
  - Incremented when a task is first dispatched (`TaskStatus::Dispatched`).
- `orchestrator_task_duration_seconds` (histogram)
  - Records end-to-end task duration from initial dispatch to a terminal state (`Succeeded` or `DeadLettered`).
- `agent_call_duration_seconds` (histogram)
  - Records the duration of each individual agent process invocation (each attempt).
- `agent_call_failures_total` (counter)
  - Incremented whenever an agent call returns an error (process failure, timeout, or invalid response).

These metrics are recorded regardless of whether the failure is ultimately retried or becomes a terminal error.

### 2.3 HTTP Endpoints

Emitted from [`Rust.mod http`](core/master_orchestrator/src/api/http.rs:1):

- `http_requests_total_chat_legacy` (counter)
  - Incremented for each call to `POST /api/chat`.
- `http_requests_total_chat_v1` (counter)
  - Incremented for each call to `POST /api/v1/chat`.
- `http_requests_total_agents_v1` (counter)
  - Incremented for each call to `GET /api/v1/agents`.
- `http_requests_total_health` (counter)
  - Incremented for each call to `GET /health`.

These counters are incremented after minimal bearer-token auth has passed.

### 2.4 Load Testing

Emitted from [`Rust.bin load_tester`](tools/load_tester/src/main.rs:1):

- `load_tester_requests_total` (counter)
  - Incremented for each HTTP request issued by the load tester.
- `load_tester_request_duration_seconds` (histogram)
  - Per-request latency from the load tester’s point of view.

---

## 3. Prometheus Configuration

The metrics exporter bind address is configured via the `METRICS_ADDR` environment variable (for example `127.0.0.1:9000`) and passed to [`Rust.fn init_metrics()`](core/platform/src/metrics.rs:18) in [`Rust.fn main()`](core/master_orchestrator/src/main.rs:89).

Example Prometheus `scrape_config`:

```yaml
scrape_configs:
  - job_name: "phoenix-orchestrator"
    scrape_interval: 15s
    static_configs:
      - targets:
          - "127.0.0.1:9000"
```

After deployment, you should be able to visit `http://METRICS_ADDR/metrics` and see all of the metrics listed above.

---

## 4. Correlation IDs and Tracing

### 4.1 HTTP and Planner/Executor

- Incoming `POST /api/v1/chat` requests are deserialized into [`Rust.struct ChatRequestV1`](core/shared_types/src/lib.rs:307) in [`Rust.fn chat_v1()`](core/master_orchestrator/src/api/http.rs:104).
- If `correlation_id` is omitted, [`Rust.fn handle_chat()`](core/master_orchestrator/src/api/http.rs:117) assigns a random UUID.
- That `correlation_id` is passed into [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221) and from there into [`Rust.fn execute_agent_for_task()`](core/master_orchestrator/src/executor.rs:434).

Planner and executor both construct scoped spans using [`Rust.fn correlation_span()`](core/platform/src/tracing.rs:1):

- `plan_and_execute_v1` creates a span named `"plan_and_execute_v1"`.
- `execute_agent_for_task` creates a span named `"execute_agent_for_task"` and logs:
  - `agent`
  - `plan_id`
  - `task_id`
  - `correlation_id`

This makes it possible to:

1. Start from a `correlation_id` observed in the HTTP response.
2. Filter logs in your log backend by that `correlation_id`.
3. See both planner and executor events for that specific request.

### 4.2 Agent Health and Circuit Breakers

Agent health metrics themselves are stored in SQLite and surfaced via HTTP rather than as Prometheus metrics:

- [`Rust.fn list_agent_health()`](core/master_orchestrator/src/memory_service.rs:533) returns all [`Rust.struct AgentHealthSummaryV1`](core/shared_types/src/lib.rs:271) rows.
- [`Rust.fn list_agents()`](core/master_orchestrator/src/api/http.rs:162) exposes `GET /api/v1/agents`, which returns these summaries.

The planner consults this state in [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:305) and will fail fast with `OrchestratorErrorCode::AgentUnavailable` when an agent is `Unhealthy` with a future `circuit_open_until`. This behavior is reflected in logs through both plan state changes and the associated `correlation_id`.

---

## 5. Example Dashboards

The exact dashboards will depend on your monitoring stack, but the following panels are recommended:

1. **Plan Success Rate**
   - `rate(orchestrator_plan_succeeded_total[5m]) / rate(orchestrator_plan_started_total[5m])`
   - Shows overall success vs failure rate over time.

2. **Task Latency**
   - Histogram or heatmap of `orchestrator_task_duration_seconds`.
   - 50th/90th/99th percentile task durations by time window.

3. **Agent Reliability**
   - Time series of `agent_call_failures_total` and `agent_call_duration_seconds`.
   - Overlay with alerts when failure rate spikes or latency degrades.

4. **HTTP Traffic**
   - Separate series for:
     - `http_requests_total_chat_v1`
     - `http_requests_total_chat_legacy`
     - `http_requests_total_agents_v1`
     - `http_requests_total_health`
   - Helps distinguish usage of legacy vs v1 endpoints and monitors health endpoint traffic.

5. **Load Testing**
   - Panels for:
     - `load_tester_requests_total`
     - `load_tester_request_duration_seconds`
   - Use when running the [`Rust.bin load_tester`](tools/load_tester/src/main.rs:1) tool to validate SLOs.

---

## 6. Troubleshooting Guide

### 6.1 A Chat Request Failed or Timed Out

1. **Identify the correlation ID**
   - The frontend displays `correlation_id` for v1 responses.
   - For API clients, read it from the `ChatResponseV1` body.

2. **Trace through logs**
   - Filter logs by this `correlation_id`.
   - Look for:
     - Planner logs from [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221) indicating plan state transitions.
     - Executor logs from [`Rust.fn execute_agent_for_task()`](core/master_orchestrator/src/executor.rs:434) indicating which agent was invoked and with what result.

3. **Check metrics**
   - Inspect `orchestrator_plan_failed_total` and `agent_call_failures_total` for spikes.
   - Look at `orchestrator_task_duration_seconds` and `agent_call_duration_seconds` for increased latency.

### 6.2 Agents Are Failing Frequently

1. **Inspect `/api/v1/agents`**
   - Call `GET /api/v1/agents` (with bearer token if configured).
   - Look for agents with:
     - `health = "degraded"` or `"unhealthy"`.
     - High `consecutive_failures`.
     - Non-`None` `circuit_open_until`.

2. **Confirm Circuit-Breaker Behavior**
   - If `health = "unhealthy"` and `circuit_open_until` is in the future, the planner will short-circuit with `AgentUnavailable`.
   - If the timestamp is in the past, the planner will allow a new attempt.

3. **Correlate with Metrics**
   - Check `agent_call_failures_total` and `agent_call_duration_seconds` for the affected time window.
   - Validate whether failures coincide with upstream provider issues or configuration changes.

### 6.3 Verifying Metrics Exporter

1. Ensure `METRICS_ADDR` is set (for example `127.0.0.1:9000`) in `.env` and that the orchestrator logs:

   - `"[INFO] Metrics exporter listening on 127.0.0.1:9000"`

2. Curl the endpoint:

   ```bash
   curl http://127.0.0.1:9000/metrics
   ```

3. Confirm the presence of keys such as:

   - `orchestrator_plan_started_total`
   - `orchestrator_task_duration_seconds_sum`
   - `agent_call_failures_total`
   - `http_requests_total_chat_v1`

---

This document will evolve as new metrics, traces, or logging conventions are added. For any changes to metric names or semantics, update both this file and any dashboards or alerts that depend on them.