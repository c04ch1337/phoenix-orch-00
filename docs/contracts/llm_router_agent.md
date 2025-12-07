# LLM Router Agent Contract (v1)

This document defines the stable contract between the Master Orchestrator and the `llm_router_agent` binary.

All shared contract types are defined in [`core/shared_types/src/lib.rs`](core/shared_types/src/lib.rs:1), in particular:

- [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31)
- [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72)
- [`Rust.struct ActionResult`](core/shared_types/src/lib.rs:65)
- [`Rust.enum ApiVersion`](core/shared_types/src/lib.rs:10)
- [`Rust.type CorrelationId`](core/shared_types/src/lib.rs:25)
- [`Rust.struct LlmRouterConfigV1`](core/shared_types/src/lib.rs:308)
- [`Rust.struct LlmRouterInvocationRequestV1`](core/shared_types/src/lib.rs:316)

The LLM Router Agent adheres to these shared contracts and expects a specific payload schema in `ActionRequest.payload`.

---

## 1. Transport and process-level contract

### 1.1 Invocation

- The orchestrator starts the agent as a child process.
- The agent reads exactly one JSON `ActionRequest` from STDIN.
- The agent writes exactly one JSON `ActionResponse` to STDOUT, then exits with code `0` on success, or non-zero on fatal error.

The orchestrator validates the response against the JSON Schema implied by [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) before using it.

### 1.2 `ActionRequest` fields

For LLM router invocations, the orchestrator populates [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) as follows:

- `request_id: uuid::Uuid` — unique per invocation.
- `api_version: Option<ApiVersion>`  
  - Currently `None` or `Some(ApiVersion::V1)`.
- `tool: String`  
  - Must be `"llm_router_agent"` for this agent.
- `action: String`  
  - For v1: `"execute"`.
- `context: String`  
  - Natural-language context and retrieved memory; can be empty.
- `plan_id: Option<PlanId>`  
  - Set by orchestrator when part of a plan, else `None`.
- `task_id: Option<TaskId>`  
  - Set by orchestrator when part of a task, else `None`.
- `correlation_id: Option<CorrelationId>`  
  - Used for cross-service tracing; non-`None` in v1 paths.
- `payload: Payload`  
  - JSON body whose shape for this agent is described below.

---

## 2. LLM Router domain payload (v1)

The logical payload for this agent is modeled as:

- [`Rust.struct LlmRouterConfigV1`](core/shared_types/src/lib.rs:308)
- [`Rust.struct LlmRouterInvocationRequestV1`](core/shared_types/src/lib.rs:316)

### 2.1 `LlmRouterConfigV1`

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRouterConfigV1 {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: String,
}
```

Semantics:

- `provider` — logical provider name, e.g. `"openrouter"`, `"openai"`, `"gemini"`.
- `api_key` — opaque API key. The orchestrator **may** redact this during logging.
- `base_url` — HTTP endpoint base URL for the provider.
- `model_name` — provider-specific model identifier.

### 2.2 `LlmRouterInvocationRequestV1`

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRouterInvocationRequestV1 {
    pub prompt: String,
    pub config: LlmRouterConfigV1,
}
```

Semantics:

- `prompt` — fully assembled user + context prompt (the orchestrator may inject retrieved memory).
- `config` — which provider/model to call and how.

### 2.3 Mapping into `ActionRequest.payload`

The orchestrator currently encodes the domain payload into `ActionRequest.payload.0` as a JSON object matching `LlmRouterInvocationRequestV1`:

```jsonc
{
  "prompt": "user-visible prompt with retrieved context",
  "config": {
    "provider": "openrouter",
    "api_key": "sk-***",
    "base_url": "https://openrouter.ai/api/v1",
    "model_name": "google/gemini-2.0-flash-exp:free"
  }
}
```

The LLM Router Agent implementation in [`agents/llm_router_agent/src/main.rs`](agents/llm_router_agent/src/main.rs:1) currently accesses this as:

- `request.payload.0["prompt"]` — string
- `request.payload.0["config"]` — object with `api_key`, `base_url`, `model_name`, and optionally `provider`

This is intentionally aligned with [`Rust.struct LlmRouterInvocationRequestV1`](core/shared_types/src/lib.rs:316).

---

## 3. Response contract

### 3.1 `ActionResponse` for success

On successful LLM completion, the agent returns an [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) where:

- `request_id` — must **echo** the `ActionRequest.request_id`.
- `api_version` — currently `None` (or `Some(ApiVersion::V1)` if/when upgraded).
- `status` — `"success"`.
- `code` — `0`.
- `result: Option<ActionResult>` — `Some(...)`, where [`Rust.struct ActionResult`](core/shared_types/src/lib.rs:65) is:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionResult {
    pub output_type: String,
    pub data: String,
    pub metadata: Option<serde_json::Value>,
}
```

For the LLM Router Agent:

- `output_type` — `"text"` for a normal completion.
- `data` — the textual content returned by the model.
- `metadata` — optional JSON, e.g.:

```json
{
  "provider": "openrouter",
  "model": "google/gemini-2.0-flash-exp:free"
}
```

- `error` — `None`.
- `plan_id` — propagated from request (if present).
- `task_id` — propagated from request (if present).
- `correlation_id` — propagated from request (if present).

Concrete example:

```json
{
  "request_id": "f15c14e1-1c5e-4f09-9dbf-0e04c0bd7c5e",
  "api_version": null,
  "status": "success",
  "code": 0,
  "result": {
    "output_type": "text",
    "data": "Here is your answer...",
    "metadata": {
      "provider": "openrouter",
      "model": "google/gemini-2.0-flash-exp:free"
    }
  },
  "error": null,
  "plan_id": "7a476b46-3b58-4b09-9afb-1b4c7d9642ce",
  "task_id": "aa0a3c3d-1b01-4df7-a96e-4081e2a0d765",
  "correlation_id": "8408fdd8-327a-4c26-9c79-8a8d51d8ab0e"
}
```

### 3.2 `ActionResponse` for logical errors

If the LLM call fails in a recoverable/logical way (e.g. HTTP 4xx, provider error), the agent should:

- Still exit with process code `0`.
- Return `status: "success"` but encode the failure as an `"error"`-typed result **or**
- Return `status: "error"` and set `code != 0` and `error: Some(String)`.

The current implementation prefers returning `"error"` in `ActionResult.output_type` while keeping `status: "success"`. The orchestrator treats this as a successful invocation with a domain-level error.

Example (provider error):

```json
{
  "request_id": "f15c14e1-1c5e-4f09-9dbf-0e04c0bd7c5e",
  "status": "success",
  "code": 0,
  "result": {
    "output_type": "error",
    "data": "LLM Call Failed: API Error 401: ...",
    "metadata": null
  },
  "error": null
}
```

### 3.3 Fatal/transport errors

If the agent cannot parse the input JSON or encounters an unrecoverable runtime error before building an `ActionResponse`, it may:

- Write to STDERR for diagnostics.
- Exit with a non-zero process code.

The orchestrator then maps this into a [`Rust.enum ToolError`](core/shared_types/src/lib.rs:142) and, in v1 flow, into an [`Rust.struct OrchestratorError`](core/shared_types/src/lib.rs:196) with `code: OrchestratorErrorCode::ExecutionFailed`.

---

## 4. Versioning and correlation

- `api_version` in `ActionRequest` / `ActionResponse` is reserved for protocol upgrades.
- `correlation_id` (when present) is a [`Rust.type CorrelationId`](core/shared_types/src/lib.rs:25) propagated from user request through orchestrator to this agent.
- The new `platform` crate provides `correlation_span` in [`Rust.fn correlation_span`](core/platform/src/tracing.rs:10) which can be used to create structured spans tied to `correlation_id` in future enhancements.

The agent must always echo `request_id` and SHOULD preserve `plan_id`, `task_id`, and `correlation_id` when responding to enable consistent end-to-end tracing.