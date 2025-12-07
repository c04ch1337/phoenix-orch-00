# Bare-Metal Master Orchestrator

This project is a bare-metal Master Orchestrator written in Rust, designed for longevity and extensibility. It uses a Cargo workspace to manage the core orchestrator, shared contracts, and standalone agents.

The orchestrator exposes:

- HTTP/WS APIs for chat and plan execution.
- A set of JSON-over-STDIO agents (LLM router, Git, Obsidian).
- A memory layer backed by SQLite (structured) and Sled (semantic).
- A small, hardened frontend for interacting with `/api/chat` and `/api/v1/chat`.

---

## Architecture Overview

### High-Level Components

- **Core**
  - [`master_orchestrator`](core/master_orchestrator/Cargo.toml:1)
    - HTTP/WS API boundary in [`Rust.mod api`](core/master_orchestrator/src/api/mod.rs:1)
    - Planner in [`Rust.fn plan_and_execute_v1()`](core/master_orchestrator/src/planner.rs:221)
    - Executor in [`Rust.fn execute_agent_for_task()`](core/master_orchestrator/src/executor.rs:434)
    - Memory and health tracking in [`Rust.struct MemoryService`](core/master_orchestrator/src/memory_service.rs:31)
  - [`shared_types`](core/shared_types/src/lib.rs:1)
    - Universal contracts:
      - `ActionRequest` / `ActionResponse`
      - `ChatRequestV1` / `ChatResponseV1`
      - `AgentExecutionConfig`, `AgentHealthSummaryV1`, plan/task status enums

- **Agents**
  - [`llm_router_agent`](agents/llm_router_agent/src/main.rs:1)
    - Routes prompts to an LLM provider (e.g., OpenRouter, OpenAI) using a static `reqwest::Client`.
  - [`git_agent`](agents/git_agent/src/main.rs:1)
    - Performs safe Git operations rooted at `GIT_AGENT_REPO_ROOT`.
  - [`obsidian_agent`](agents/obsidian_agent/src/main.rs:1)
    - Performs safe file operations in an Obsidian vault rooted at `OBSIDIAN_AGENT_VAULT_ROOT`.

- **Memory & State**
  - [`MemoryService`](core/master_orchestrator/src/memory_service.rs:31)
    - Agent registry (SQLite)
    - Action trace log (SQLite)
    - Knowledge graph / structured memory (SQLite)
    - Semantic memory via Sled in [`Rust.mod semantic`](core/master_orchestrator/src/memory/semantic.rs:1)
    - Agent health + circuit breaker state (`agent_health` table)
  - Streaming helpers in [`Rust.mod io`](core/master_orchestrator/src/memory/io.rs:1) for future large I/O.

- **Frontend**
  - Static assets in [`frontend/`](frontend/index.html:1)
    - [`index.html`](frontend/index.html:1)
    - [`script.js`](frontend/script.js:1)
    - [`style.css`](frontend/style.css:1)
  - Hardened UI:
    - DOM-safe rendering via [`Rust.fn setSafeContent`](frontend/script.js:156)
    - Simple state machine for idle/loading/error/degraded
    - Endpoint fallback between `/api/v1/chat` and `/api/chat`
    - Correlation ID display for tracing.

- **Platform**
  - [`platform`](core/platform/src/lib.rs:1)
    - Tracing initialization and `correlation_span`
    - Metrics exporter via Prometheus (`init_metrics`, `record_counter`, `record_histogram`)
    - Common error helpers

### Architecture Diagram

```mermaid
flowchart LR
  user[User / Browser] --> ui[Frontend (index.html, script.js)]
  ui --> http[/HTTP API (Actix Web)\n/api/chat, /api/v1/chat, /health, /api/v1/agents/]

  http --> planner[Planner\nplan_and_execute_v1]
  planner --> executor[Executor\nexecute_agent_for_task / with_retries]

  executor -->|JSON over STDIO| agents[Agents\nllm_router_agent\ngit_agent\nobsidian_agent]

  planner --> memory[(MemoryService\nSQLite + Sled)]
  executor --> memory
  agents --> memory

  subgraph Data & Config
    cfg[Config\n(data/config*.toml)\nAppConfig & AgentsConfig]
  end
  cfg --> planner
  cfg --> executor
  cfg --> agents
```

---

## Quickstart for New Engineers

### Prerequisites

- Rust toolchain (stable; e.g. via `rustup`)
- Node.js + npm (for frontend tests)
- SQLite (used via `rusqlite`, no manual setup required)
- Git (for `git_agent`)
- Access to an LLM provider (e.g. OpenRouter) with an API key

### 1. Clone and Build

```bash
git clone <this-repo-url>
cd <repo-root>

# Build everything
cargo build --workspace
```

### 2. Configure Environment

1. Copy and edit `.env.example`:

   ```bash
   cp .env.example .env
   ```

2. Set at least:

   - `OPENROUTER_API_KEY` (or keys for your chosen provider)
   - `ORCH_API_TOKEN` (a random secret string)
   - `APP_ENV=dev` (or `staging` / `prod`)
   - `GIT_AGENT_REPO_ROOT` (path to a repo for `git_agent`)
   - `OBSIDIAN_AGENT_VAULT_ROOT` (path to a vault for `obsidian_agent`)

3. Optionally configure metrics:

   - `METRICS_ADDR=127.0.0.1:9000`

Configuration files:

- Base config: [`data/config.toml`](data/config.toml:1)
- Environment overlays:
  - [`data/config.dev.toml`](data/config.dev.toml:1)
  - [`data/config.staging.toml`](data/config.staging.toml:1)
  - [`data/config.prod.toml`](data/config.prod.toml:1)

These are merged by [`Rust.fn load_app_config_with_env()`](core/master_orchestrator/src/config_service.rs:149).

### 3. Run the Orchestrator

From the repo root:

```bash
cd core/master_orchestrator
cargo run --bin master_orchestrator
```

This will:

- Load config with `APP_ENV`.
- Initialize SQLite + Sled memory.
- Register agents in the agent registry.
- Start the HTTP server on `127.0.0.1:8181`.
- Start the metrics exporter on `METRICS_ADDR` (if valid).

Key endpoints:

- `POST /api/chat` – legacy chat endpoint.
- `POST /api/v1/chat` – versioned chat endpoint using `ChatRequestV1` / `ChatResponseV1`.
- `GET  /api/v1/agents` – agent health summaries.
- `GET  /health` – simple health probe with LLM provider/model info.

All protected by minimal bearer auth when `ORCH_API_TOKEN` is set.

### 4. Use the Frontend

The orchestrator serves static files from `frontend/` when built and run from `core/master_orchestrator`, but during development it’s often easiest to open the HTML file directly:

- Open [`frontend/index.html`](frontend/index.html:1) in a browser, or
- Serve it via a simple static server in the repo root:

  ```bash
  cd frontend
  npx serve .
  ```

Ensure your browser can reach `http://127.0.0.1:8181` and that the frontend is configured with the correct origin (by default it points at `http://127.0.0.1:8181` for API calls).

### 5. Run Tests

#### Rust (Workspace)

From the repo root:

```bash
cargo fmt --all
cargo test --workspace
```

This runs:

- Shared types unit tests (contracts, defaults, round-trips).
- Planner/executor tests (retry, backoff, circuit helper).
- Config service tests (env substitution, config merging).
- Integration smoke test:
  - [`core/master_orchestrator/tests/smoke_chat_v1.rs`](core/master_orchestrator/tests/smoke_chat_v1.rs:1)

#### Frontend (Jest)

```bash
cd frontend
npm install
npm test
```

Frontend tests live in:

- [`frontend/tests/script.test.js`](frontend/tests/script.test.js:1)

They validate the DOM-safe content helpers and message rendering in [`frontend/script.js`](frontend/script.js:1).

---

## Key Documentation

- **Resilience & Failure Handling**
  - [`docs/resilience_strategy.md`](docs/resilience_strategy.md:1)

- **Agent Contracts**
  - [`docs/contracts/llm_router_agent.md`](docs/contracts/llm_router_agent.md:1)
  - [`docs/contracts/git_agent.md`](docs/contracts/git_agent.md:1)
  - [`docs/contracts/obsidian_agent.md`](docs/contracts/obsidian_agent.md:1)

- **Observability**
  - [`docs/observability.md`](docs/observability.md:1)

- **Runbooks**
  - Deployment: [`docs/runbooks/deploy.md`](docs/runbooks/deploy.md:1) (once created)
  - Rollback: [`docs/runbooks/rollback.md`](docs/runbooks/rollback.md:1)
  - Agent incidents: [`docs/runbooks/incident_agent_failure.md`](docs/runbooks/incident_agent_failure.md:1)

- **Changelog**
  - [`CHANGELOG.md`](CHANGELOG.md:1) tracks versions and production-hardening changes.

---

## Development Notes

- Agents communicate with the orchestrator via JSON over STDIO and are invoked by the executor using [`Rust.fn execute_agent()`](core/master_orchestrator/src/executor.rs:144).
- Agent resilience is controlled by:
  - [`Rust.struct AgentExecutionConfig`](core/shared_types/src/lib.rs:138)
  - Retry logic in [`Rust.fn execute_agent_with_retries()`](core/master_orchestrator/src/executor.rs:218)
  - Circuit breaker state in [`Rust.fn update_agent_health_on_failure()`](core/master_orchestrator/src/memory_service.rs:384) and consulted by the planner.
- All public HTTP/WS surfaces share:
  - Minimal bearer-token auth in [`Rust.fn require_auth()`](core/master_orchestrator/src/api/http.rs:34)
  - Baseline security headers configured in [`Rust.fn run_http_server()`](core/master_orchestrator/src/main.rs:40).

This README is intentionally high-level. For deeper implementation details, refer to the inline Rust documentation and the docs referenced above.
