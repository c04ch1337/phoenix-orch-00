# Git Agent Contract (v1)

This document defines the stable contract between the Master Orchestrator and the `git_agent` binary.

All shared contract types are defined in [`core/shared_types/src/lib.rs`](core/shared_types/src/lib.rs:1), most importantly:

- [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31)
- [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72)
- [`Rust.struct ActionResult`](core/shared_types/src/lib.rs:65)
- [`Rust.enum ApiVersion`](core/shared_types/src/lib.rs:10)
- [`Rust.type PlanId`](core/shared_types/src/lib.rs:19)
- [`Rust.type TaskId`](core/shared_types/src/lib.rs:20)
- [`Rust.type CorrelationId`](core/shared_types/src/lib.rs:25)
- [`Rust.enum GitAgentCommandV1`](core/shared_types/src/lib.rs:326)
- [`Rust.struct GitAgentRequestV1`](core/shared_types/src/lib.rs:339)

The Git Agent adheres to these shared contracts and expects a specific command-oriented payload schema in `ActionRequest.payload`.

The current reference implementation lives in [`agents/git_agent/src/main.rs`](agents/git_agent/src/main.rs:1).

---

## 1. Transport and process-level contract

### 1.1 Invocation

- The orchestrator starts `git_agent` as a child process (e.g., `git_agent.exe` on Windows).
- The agent reads exactly one JSON [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) from STDIN.
- The agent writes exactly one JSON [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) to STDOUT and exits.

The orchestrator validates the response against the JSON Schema implied by [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72) before using it.

### 1.2 `ActionRequest` fields

For git-related invocations, the orchestrator populates [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) as follows:

- `request_id: Uuid`  
  - Unique per invocation.
- `api_version: Option<ApiVersion>`  
  - Currently `None` or `Some(ApiVersion::V1)`.
- `tool: String`  
  - Must be `"git_agent"` for this agent.
- `action: String`  
  - One of:
    - `"git_status"`
    - `"git_diff"`
    - `"git_log"`
    - `"git_add"`
    - `"git_commit"`
- `context: String`  
  - Optional natural-language context; not used directly by the agent in v1.
- `plan_id: Option<PlanId>`  
  - Set by orchestrator when called as part of a plan.
- `task_id: Option<TaskId>`  
  - Set by orchestrator for per-task tracking.
- `correlation_id: Option<CorrelationId>`  
  - Used for distributed tracing.
- `payload: Payload`  
  - JSON body whose shape depends on the `action`, aligned with [`Rust.enum GitAgentCommandV1`](core/shared_types/src/lib.rs:326).

---

## 2. Git Agent domain commands (v1)

The logical git command model is:

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GitAgentCommandV1 {
    GitStatus,
    GitDiff { files: Vec<String> },
    GitLog {
        #[serde(default = "default_git_log_limit")]
        limit: u32,
    },
    GitAdd { files: Vec<String> },
    GitCommit { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GitAgentRequestV1 {
    pub command: GitAgentCommandV1,
}
```

The current `git_agent` implementation in [`agents/git_agent/src/main.rs`](agents/git_agent/src/main.rs:39) uses simpler ad-hoc payloads keyed by `ActionRequest.action`. This contract document defines the **canonical** v1 structure; the current implementation is compatible with it but does not yet deserialize `GitAgentRequestV1` directly.

### 2.1 Action mapping

The mapping between `ActionRequest.action` and `GitAgentCommandV1` is:

| `action` string | Command variant                 | Payload fields                          |
|-----------------|---------------------------------|-----------------------------------------|
| `"git_status"`  | `GitAgentCommandV1::GitStatus` | *(none)*                                |
| `"git_diff"`    | `GitAgentCommandV1::GitDiff`   | `files: Vec<String>`                |
| `"git_log"`     | `GitAgentCommandV1::GitLog`    | `limit: u32` (default `10`)             |
| `"git_add"`     | `GitAgentCommandV1::GitAdd`    | `files: Vec<String>`                |
| `"git_commit"`  | `GitAgentCommandV1::GitCommit` | `message: String`                        |

### 2.2 Payload JSON shapes

The agent currently reads parameters from `ActionRequest.payload.0` as follows:

#### `git_status`

```jsonc
{
  // payload may be empty for status
}
```

(No fields are read; the agent executes `git status`.)

#### `git_diff`

```jsonc
{
  "files": ["src/main.rs", "Cargo.toml"]
}
```

The implementation uses:

```rust
let files: Vec<String> =
    request.payload.0.get("files")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
```

#### `git_log`

```jsonc
{
  "limit": "20"
}
```

The current implementation expects a **string** limit:

```rust
let limit = request.payload.0
    .get("limit")
    .and_then(Value::as_str)
    .unwrap_or("10");
```

This differs slightly from the canonical [`Rust.enum GitAgentCommandV1::GitLog`](core/shared_types/src/lib.rs:331) which uses `u32`; future revisions can harmonize this by parsing numeric values.

#### `git_add`

```jsonc
{
  "files": ["src/main.rs", "README.md"]
}
```

Implementation:

```rust
let files: Vec<String> = match request.payload.0.get("files") {
    Some(Value::Array(arr)) => arr.iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect(),
    _ => vec![]
};
```

If `files` is empty, `ActionResult` is an error.

#### `git_commit`

```jsonc
{
  "message": "feat: add new planner"
}
```

Implementation:

```rust
let message = request.payload.0
    .get("message")
    .and_then(Value::as_str);
```

If `message` is missing, an error `ActionResult` is returned.

---

## 3. Response contract

### 3.1 `ActionResponse` structure

On completion, the agent returns an [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72):

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionResponse {
    pub request_id: Uuid,
    pub api_version: Option<ApiVersion>,
    pub status: String,
    pub code: u16,
    pub result: Option<ActionResult>,
    pub error: Option<String>,
    pub plan_id: Option<PlanId>,
    pub task_id: Option<TaskId>,
    pub correlation_id: Option<CorrelationId>,
}
```

For the Git Agent in [`agents/git_agent/src/main.rs`](agents/git_agent/src/main.rs:151):

- `request_id` — echoes `ActionRequest.request_id`.
- `api_version` — currently `None`.
- `status`:
  - `"success"` if the `git` command executed successfully and produced a non-error `ActionResult`.
  - `"error"` otherwise.
- `code`:
  - `0` for success.
  - `1` for logical or execution error.
- `result` — always `Some(ActionResult)`; on error, `output_type == "error"`.
- `error` — currently `None`; error details are carried in `result.data`.
- `plan_id` — propagated from `request.plan_id`.
- `task_id` — propagated from `request.task_id`.
- `correlation_id` — propagated from `request.correlation_id`.

### 3.2 `ActionResult` semantics

The helper [`Rust.fn execute_git_command`](agents/git_agent/src/main.rs:169) returns [`Rust.struct ActionResult`](core/shared_types/src/lib.rs:65):

- On success (git exit code `0`):
  - `output_type: "text"`
  - `data`: stdout of `git` command.
  - `metadata: None`.
- On failure (non-zero git exit code or I/O failure):
  - `output_type: "error"`
  - `data`: stderr or error string.
  - `metadata`: may include `exit_code`:

    ```json
    {
      "exit_code": 128
    }
    ```

Example success response:

```json
{
  "request_id": "f5e93cc3-5d6f-4e9e-88a2-2f10e1bf7a21",
  "api_version": null,
  "status": "success",
  "code": 0,
  "result": {
    "output_type": "text",
    "data": "On branch main\nnothing to commit, working tree clean\n",
    "metadata": null
  },
  "error": null,
  "plan_id": "7a476b46-3b58-4b09-9afb-1b4c7d9642ce",
  "task_id": "aa0a3c3d-1b01-4df7-a96e-4081e2a0d765",
  "correlation_id": "8408fdd8-327a-4c26-9c79-8a8d51d8ab0e"
}
```

Example error response:

```json
{
  "request_id": "f5e93cc3-5d6f-4e9e-88a2-2f10e1bf7a21",
  "api_version": null,
  "status": "error",
  "code": 1,
  "result": {
    "output_type": "error",
    "data": "fatal: not a git repository (or any of the parent directories): .git\n",
    "metadata": {
      "exit_code": 128
    }
  },
  "error": null,
  "plan_id": "7a476b46-3b58-4b09-9afb-1b4c7d9642ce",
  "task_id": "aa0a3c3d-1b01-4df7-a96e-4081e2a0d765",
  "correlation_id": "8408fdd8-327a-4c26-9c79-8a8d51d8ab0e"
}
```

---

## 4. Registry integration

The Git Agent also acts as its own tool registry registrar using the local SQLite DB. See [`agents/git_agent/src/main.rs`](agents/git_agent/src/main.rs:39):

- On startup, it opens `../../data/memory.db`.
- Builds a [`Rust.struct Tool`](agents/git_agent/src/main.rs:8) (different from the orchestrator’s shared `Tool` in [`core/shared_types/src/lib.rs`](core/shared_types/src/lib.rs:130)).
- Calls [`Rust.fn register_tool`](agents/git_agent/src/main.rs:20) to insert/update the record into the `tool_registry` table.

This registration is orthogonal to the `ActionRequest`/`ActionResponse` contract, but it establishes the existence of `git_agent` as a discoverable tool in the orchestrator’s tool registry.

---

## 5. Versioning and tracing

- `api_version` is reserved for future protocol upgrades (`ApiVersion::V1` and beyond).
- `plan_id`, `task_id`, and `correlation_id` are managed by the orchestrator lifecycle and are preserved by the agent for traceability.
- The shared `platform` crate defined in [`core/platform/src/lib.rs`](core/platform/src/lib.rs:1) is used to initialize structured logging/tracing in the agent’s `main`, via:

  ```rust
  platform::init_tracing("git_agent").expect("failed to init tracing");
  ```

This makes the agent’s logs compatible with orchestrator-level correlation using `correlation_id` fields propagated through [`Rust.struct ActionRequest`](core/shared_types/src/lib.rs:31) and [`Rust.struct ActionResponse`](core/shared_types/src/lib.rs:72).