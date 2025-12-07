# Changelog

All notable changes to this project will be documented in this file.

The format is inspired by [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- Further refinements to resilience, observability, and testing.
- Additional docs and runbooks as the system evolves.

### Changed
- Pending future changes.

### Fixed
- Pending future fixes.

---

## [0.2.0] - 2025-12-07

### Added
- **Resilience & Failure Handling**
  - Shared `AgentExecutionConfig`, `AgentRetryConfig`, and `AgentCircuitBreakerConfig` in [`core/shared_types/src/lib.rs`](core/shared_types/src/lib.rs:125).
  - Exponential backoff and retry logic in [`Rust.fn execute_agent_with_retries()`](core/master_orchestrator/src/executor.rs:218) with configurable timeouts and circuit-breaker thresholds.
  - Agent health tracking and circuit breakers via `agent_health` table and:
    - [`Rust.fn update_agent_health_on_success()`](core/master_orchestrator/src/memory_service.rs:352)
    - [`Rust.fn update_agent_health_on_failure()`](core/master_orchestrator/src/memory_service.rs:384)
    - [`Rust.fn get_agent_health()`](core/master_orchestrator/src/memory_service.rs:479)
  - Planner-side short-circuiting of unhealthy agents in [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221).

- **Configuration & Environment Profiles**
  - Environment-specific config overlays:
    - [`data/config.dev.toml`](data/config.dev.toml:1)
    - [`data/config.staging.toml`](data/config.staging.toml:1)
    - [`data/config.prod.toml`](data/config.prod.toml:1)
  - Env-aware loader [`Rust.fn load_app_config_with_env()`](core/master_orchestrator/src/config_service.rs:149).
  - Expanded `.env.example` documenting required/optional env vars for orchestrator, metrics, and agents.

- **Performance & Helpers**
  - Global agent concurrency limit via `AGENT_CONCURRENCY` semaphore in [`core/master_orchestrator/src/executor.rs`](core/master_orchestrator/src/executor.rs:95).
  - Streaming I/O helpers in [`Rust.mod io`](core/master_orchestrator/src/memory/io.rs:1).
  - Static `reqwest::Client` for `llm_router_agent` in [`agents/llm_router_agent/src/main.rs`](agents/llm_router_agent/src/main.rs:1).
  - Load test harness crate:
    - [`tools/load_tester/Cargo.toml`](tools/load_tester/Cargo.toml:1)
    - [`tools/load_tester/src/main.rs`](tools/load_tester/src/main.rs:1)

- **Metrics & Observability**
  - Prometheus metrics exporter via [`Rust.fn init_metrics()`](core/platform/src/metrics.rs:18).
  - Metric helpers:
    - [`Rust.fn record_counter()`](core/platform/src/metrics.rs:35)
    - [`Rust.fn record_histogram()`](core/platform/src/metrics.rs:40)
  - Planner and executor metrics:
    - `orchestrator_plan_started_total`
    - `orchestrator_plan_succeeded_total`
    - `orchestrator_plan_failed_total`
    - `orchestrator_task_started_total`
    - `orchestrator_task_duration_seconds`
    - `agent_call_duration_seconds`
    - `agent_call_failures_total`
  - HTTP metrics:
    - `http_requests_total_chat_legacy`
    - `http_requests_total_chat_v1`
    - `http_requests_total_agents_v1`
    - `http_requests_total_health`
  - Load test metrics:
    - `load_tester_requests_total`
    - `load_tester_request_duration_seconds`
  - Correlated tracing with `correlation_span` in:
    - [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221)
    - [`Rust.fn execute_agent_for_task()`](core/master_orchestrator/src/executor.rs:434)
  - Observability guide in [`docs/observability.md`](docs/observability.md:1).

- **Testing**
  - Shared types tests in [`Rust.mod tests`](core/shared_types/src/lib.rs:418).
  - Executor unit tests (backoff and error classification) in [`Rust.mod tests`](core/master_orchestrator/src/executor.rs:434).
  - Planner helper tests for circuit cooldown in [`Rust.mod tests`](core/master_orchestrator/src/planner.rs:505).
  - Config service tests in [`Rust.mod tests`](core/master_orchestrator/src/config_service.rs:149).
  - Integration smoke test for `/api/v1/chat` in [`core/master_orchestrator/tests/smoke_chat_v1.rs`](core/master_orchestrator/tests/smoke_chat_v1.rs:1).
  - Frontend Jest setup:
    - [`frontend/package.json`](frontend/package.json:1)
    - [`frontend/jest.config.cjs`](frontend/jest.config.cjs:1)
    - DOM helper tests in [`frontend/tests/script.test.js`](frontend/tests/script.test.js:1)
    - Node-friendly exports in [`frontend/script.js`](frontend/script.js:214).

- **Docs & Runbooks**
  - Updated resilience description in [`docs/resilience_strategy.md`](docs/resilience_strategy.md:1).
  - New observability doc [`docs/observability.md`](docs/observability.md:1).
  - Runbooks:
    - Deployment: [`docs/runbooks/deploy.md`](docs/runbooks/deploy.md:1)
    - Rollback: [`docs/runbooks/rollback.md`](docs/runbooks/rollback.md:1)
    - Agent incidents: [`docs/runbooks/incident_agent_failure.md`](docs/runbooks/incident_agent_failure.md:1)
  - Expanded project overview and quickstart in [`README.md`](README.md:1).

- **Build & Runtime**
  - Production-optimized release profile in root [`Cargo.toml`](Cargo.toml:1) under `[profile.release]`.
  - Workspace member for load tester in [`Cargo.toml`](Cargo.toml:1).
  - Library entry point for orchestrator modules in [`core/master_orchestrator/src/lib.rs`](core/master_orchestrator/src/lib.rs:1) to support integration tests.

### Changed
- Hardened HTTP layer with minimal bearer-token auth in [`Rust.fn require_auth()`](core/master_orchestrator/src/api/http.rs:34) and strict security headers in [`Rust.fn run_http_server()`](core/master_orchestrator/src/main.rs:40).
- Refined planner and executor lifecycles to consistently track:
  - Plan states: `Draft`, `Pending`, `Running`, `Succeeded`, `Failed`.
  - Task states: `Queued`, `Dispatched`, `InProgress`, `Retried`, `DeadLettered`.

### Fixed
- Ensured `ChatRequestV1` defaults `api_version` to `API_VERSION_CURRENT` when omitted, validated by tests.
- Improved robustness of agent response validation using JSON Schema in [`Rust.fn validate_and_parse_action_response()`](core/master_orchestrator/src/executor.rs:65).
- Normalized frontend behavior for both `/api/v1/chat` and legacy `/api/chat` with defensive DOM rendering and state transitions.
