# Obsidian Agent Contract (v1)

This document defines the stable contract between the Master Orchestrator and the `obsidian_agent` binary.

All shared contract types are defined in [`core/shared_types/src/lib.rs`](core/shared_types/src/lib.rs:1), most importantly:

- [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31)
- [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72)
- [`Rust.struct ActionResult`](core/shared_types/src/lib.rs:65)
- [`Rust.enum ApiVersion`](core/shared_types/src/lib.rs:10)
- [`Rust.type PlanId`](core/shared_types/src/lib.rs:19)
- [`Rust.type TaskId`](core/shared_types/src/lib.rs:20)
- [`Rust.type CorrelationId`](core/shared_types/src/lib.rs:25)
- [`Rust.enum ObsidianAgentCommandV1`](core/shared_types/src/lib.rs:344)
- [`Rust.struct ObsidianAgentRequestV1`](core/shared_types/src/lib.rs:363)

The Obsidian Agent adheres to these shared contracts and expects a specific note-oriented payload schema in `ActionRequest.payload`.

The current reference implementation lives in [`agents/obsidian_agent/src/main.rs`](agents/obsidian_agent/src/main.rs:1).

---

## 1. Transport and process-level contract

### 1.1 Invocation

- The orchestrator starts `obsidian_agent` as a child process.
- The agent reads exactly one JSON [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) from STDIN.
- The agent writes exactly one JSON [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) to STDOUT and exits.

The orchestrator validates the response against the JSON Schema implied by [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) before using it.

### 1.2 `ActionRequest` fields

For Obsidian note operations, the orchestrator populates [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) as follows:

- `request_id: Uuid`  
  - Unique per invocation.
- `api_version: Option<ApiVersion>`  
  - Currently `None` or `Some(ApiVersion::V1)`.
- `tool: String`  
  - Must be `"obsidian_agent"` for this agent.
- `action: String`  
  - One of:
    - `"create_note"`
    - `"read_note"`
    - `"update_note"`
- `context: String`  
  - Optional natural-language context; not directly consumed in v1.
- `plan_id: Option<PlanId>`  
  - Set by orchestrator when called as part of a plan.
- `task_id: Option<TaskId>`  
  - Set by orchestrator for per-task tracking.
- `correlation_id: Option<CorrelationId>`  
  - Used for distributed tracing.
- `payload: Payload`  
  - JSON body encoding note parameters, consistent with [`Rust.enum ObsidianAgentCommandV1`](core/shared_types/src/lib.rs:344).

---

## 2. Obsidian Agent domain commands (v1)

The canonical note command model is:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObsidianAgentCommandV1 {
    CreateNote {
        vault_path: String,
        note_name: String,
        content: String,
    },
    ReadNote {
        vault_path: String,
        note_name: String,
    },
    UpdateNote {
        vault_path: String,
        note_name: String,
        content: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObsidianAgentRequestV1 {
    pub command: ObsidianAgentCommandV1,
}
```

The current implementation in [`agents/obsidian_agent/src/main.rs`](agents/obsidian_agent/src/main.rs:46) uses `ActionRequest.action` to decide which operation to perform, and reads fields directly from `ActionRequest.payload.0`. This contract describes the canonical shapes consistent with [`Rust.enum ObsidianAgentCommandV1`](core/shared_types/src/lib.rs:344).

### 2.1 Action mapping

Mapping between `ActionRequest.action` and `ObsidianAgentCommandV1`:

| `action` string | Command variant                                    | Required payload fields                          |
|-----------------|----------------------------------------------------|--------------------------------------------------|
| `"create_note"` | `ObsidianAgentCommandV1::CreateNote`              | `vault_path`, `note_name`, `content`            |
| `"read_note"`   | `ObsidianAgentCommandV1::ReadNote`                | `vault_path`, `note_name`                        |
| `"update_note"` | `ObsidianAgentCommandV1::UpdateNote`              | `vault_path`, `note_name`, `content`            |

### 2.2 Payload JSON shapes

The agent reads parameters from `ActionRequest.payload.0` using raw JSON access, e.g. [`Rust.fn handle_request`](agents/obsidian_agent/src/main.rs:46).

#### Common field: `vault_path`

Every command requires:

```jsonc
{
  "vault_path": "/path/to/obsidian/vault"
}
```

If `vault_path` is missing or not a string, [`Rust.fn handle_request`](agents/obsidian_agent/src/main.rs:46) returns an `ActionResponse` with an error.

#### `create_note`

```jsonc
{
  "vault_path": "/path/to/vault",
  "note_name": "ProjectPlan",
  "content": "# Project Plan\n\n- [ ] Task 1\n"
}
```

Handled by [`Rust.fn create_note`](agents/obsidian_agent/src/main.rs:98):

- Creates `${vault_path}/${note_name}.md` with `content`.

#### `read_note`

```jsonc
{
  "vault_path": "/path/to/vault",
  "note_name": "ProjectPlan"
}
```

Handled by [`Rust.fn read_note`](agents/obsidian_agent/src/main.rs:112):

- Reads `${vault_path}/${note_name}.md` and returns its content.

#### `update_note`

```jsonc
{
  "vault_path": "/path/to/vault",
  "note_name": "ProjectPlan",
  "content": "# Project Plan\n\n- [x] Task 1\n"
}
```

Handled by [`Rust.fn update_note`](agents/obsidian_agent/src/main.rs:125):

- Overwrites existing `${vault_path}/${note_name}.md` with `content`.

---

## 3. Response contract

### 3.1 Success responses

The agent returns [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72). After v1 lifecycle wiring, success responses SHOULD:

- Echo `request_id`.
- Preserve incoming `plan_id`, `task_id`, and `correlation_id`.
- Use `status: "success"` and `code: 0`.
- Populate `result: Some(ActionResult)` with:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionResult {
    pub output_type: String,
    pub data: String,
    pub metadata: Option<serde_json::Value>,
}
```

The current implementation returns success as:

- For `create_note` ([`Rust.fn create_note`](agents/obsidian_agent/src/main.rs:98)):

```json
{
  "output_type": "text",
  "data": "Note 'ProjectPlan' created successfully.",
  "metadata": null
}
```

- For `read_note` ([`Rust.fn read_note`](agents/obsidian_agent/src/main.rs:112)):

```json
{
  "output_type": "text",
  "data": "<file contents>",
  "metadata": null
}
```

- For `update_note` ([`Rust.fn update_note`](agents/obsidian_agent/src/main.rs:125)):

```json
{
  "output_type": "text",
  "data": "Note 'ProjectPlan' updated successfully.",
  "metadata": null
}
```

And wraps them as:

```json
{
  "request_id": "<same as request>",
  "api_version": null,
  "status": "success",
  "code": 0,
  "result": { /* ActionResult as above */ },
  "error": null,
  "plan_id": "<propagated>",
  "task_id": "<propagated>",
  "correlation_id": "<propagated>"
}
```

### 3.2 Error responses

Validation and I/O errors are handled in-place:

- If the incoming request cannot be parsed, [`Rust.fn main`](agents/obsidian_agent/src/main.rs:7) returns:

```json
{
  "request_id": "<new random uuid>",
  "api_version": null,
  "status": "error",
  "code": 1,
  "result": null,
  "error": "Failed to parse request: ...",
  "plan_id": null,
  "task_id": null,
  "correlation_id": null
}
```

- If required fields like `vault_path`, `note_name`, or `content` are missing or invalid, [`Rust.fn handle_request`](agents/obsidian_agent/src/main.rs:46) returns:

```json
{
  "request_id": "<same as request>",
  "api_version": null,
  "status": "error",
  "code": 2 or 3,
  "result": null,
  "error": "Missing 'vault_path' in parameters" // or other message
}
```

For file-system errors (e.g. missing note on update), helper functions return `Err(String)`, which is wrapped into an `ActionResponse` with:

- `status: "error"`
- `code: 3`
- `error: Some(String)`

### 3.3 Plan/task propagation

With the shared v1 contracts, the agent is expected to propagate:

- `plan_id` from `ActionRequest.plan_id` into `ActionResponse.plan_id`
- `task_id` from `ActionRequest.task_id` into `ActionResponse.task_id`
- `correlation_id` from `ActionRequest.correlation_id` into `ActionResponse.correlation_id`

This enables the orchestrator to correlate note operations with specific plans and tasks.

---

## 4. Versioning and tracing

- `api_version` in `ActionRequest` / `ActionResponse` is reserved for protocol upgrades (e.g. richer note operations, streaming updates).
- `PlanId`, `TaskId`, and `CorrelationId` are lifecycle identifiers managed by the orchestrator and only echoed by the agent.
- The `platform` crate ([`core/platform/src/lib.rs`](core/platform/src/lib.rs:1)) is used to initialize structured logging in the agent’s `main`:

  ```rust
  platform::init_tracing("obsidian_agent").expect("failed to init tracing");
  ```

This ensures logs from `obsidian_agent` can be correlated with orchestrator spans using the `correlation_id` field defined in [`Rust.type CorrelationId`](core/shared_types/src/lib.rs:25) and, in future, the helper [`Rust.fn correlation_span`](core/platform/src/tracing.rs:10).