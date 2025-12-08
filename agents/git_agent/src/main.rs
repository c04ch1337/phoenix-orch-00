use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_types::{ActionRequest, ActionResponse, ActionResult, ActionError};
use std::io::{self, Read};
use std::path::Path;
use std::process::{exit, Command, Stdio};

mod safe_paths;
use crate::safe_paths::{load_repo_root_from_env, validate_repo_paths};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub version: String,
    pub description: String,
    pub executable_path: String,
    pub actions_schema: serde_json::Value,
    pub tags: String,
    pub category: String,
    pub enabled: bool,
}

pub fn register_tool(conn: &Connection, tool: &Tool) -> rusqlite::Result<()> {
    let actions_schema_json =
        serde_json::to_string(&tool.actions_schema).unwrap_or_else(|_| "{}".to_string());

    conn.execute(
        "INSERT OR REPLACE INTO tool_registry (
            name,
            version,
            description,
            executable_path,
            actions_schema,
            tags,
            category,
            enabled
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            tool.name,
            tool.version,
            tool.description,
            tool.executable_path,
            actions_schema_json,
            tool.tags,
            tool.category,
            tool.enabled
        ],
    )?;
    Ok(())
}

fn main() {
    platform::init_tracing("git_agent").expect("failed to init tracing");

    // Resolve and validate repository root from environment. If this fails,
    // we cannot safely operate and will exit with a non-zero status so the
    // orchestrator treats this as an agent startup failure.
    let repo_root = match load_repo_root_from_env() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[FATAL] {err}");
            exit(1);
        }
    };

    // Register the tool
    let db_path = "../../data/memory.db";
    let conn = Connection::open(db_path).expect("Failed to open database");
    let git_tool = Tool {
        name: "git_agent".to_string(),
        version: "0.1.0".to_string(),
        description: "An agent for interacting with git repositories.".to_string(),
        executable_path: "path/to/git_agent_executable".to_string(),
        actions_schema: serde_json::json!({
            "git_status": {
                "description": "Get the status of the git repository.",
                "payload": {}
            },
            "git_diff": {
                "description": "Get the diff of the git repository.",
                "payload": {
                    "files": "array"
                }
            },
            "git_log": {
                "description": "Get the log of the git repository.",
                "payload": {
                    "limit": "string"
                }
            },
            "git_add": {
                "description": "Add files to the git repository.",
                "payload": {
                    "files": "array"
                }
            },
            "git_commit": {
                "description": "Commit changes to the git repository.",
                "payload": {
                    "message": "string"
                }
            }
        }),
        tags: "git,vcs,source control".to_string(),
        category: "development".to_string(),
        enabled: true,
    };

    if let Err(e) = register_tool(&conn, &git_tool) {
        eprintln!("Failed to register tool: {}", e);
    }

    let mut buffer = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buffer) {
        eprintln!("Failed to read from stdin: {}", e);
        // Cannot construct a valid contract response, so just exit.
        return;
    }

    let request: ActionRequest = match serde_json::from_str(&buffer) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            // Input is not a valid ActionRequest; nothing to safely return.
            return;
        }
    };

    // Handle actions with path validation where applicable.
    match handle_request(&repo_root, request) {
        Ok(response) => {
            let response_json = match serde_json::to_string(&response) {
                Ok(json) => json,
                Err(e) => {
                    eprintln!("Failed to serialize response: {}", e);
                    format!(
                        r#"{{"request_id":"{}","status":"error","code":1,"result":null,"error":"Failed to serialize response"}}"#,
                        response.request_id
                    )
                }
            };
            print!("{}", response_json);
        }
        Err(serialized) => {
            // Already a fully serialized JSON ActionResponse string representing
            // a validation error; just emit it.
            println!("{serialized}");
        }
    }
}

/// Handle a single ActionRequest, returning either a concrete ActionResponse
/// or a pre-serialized JSON error string for validation failures that should
/// return a 4xx-style code to the orchestrator.
fn handle_request(repo_root: &Path, request: ActionRequest) -> Result<ActionResponse, String> {
    let result = match request.action.as_str() {
        "git_status" => execute_git_command(repo_root, "status", &[]),
        "git_diff" => {
            let files: Vec<String> = request
                .payload
                .0
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            // Validate all requested paths under the configured repo root.
            let validated = match validate_repo_paths(repo_root, &files) {
                Ok(paths) => paths,
                Err(err_msg) => {
                    let result = ActionResult {
                        output_type: "error".to_string(),
                        data: err_msg.clone(),
                        metadata: None,
                    };
                    let response = ActionResponse {
                        request_id: request.request_id,
                        api_version: request.api_version,
                        status: "error".to_string(),
                        code: 400,
                        result: Some(result),
                        error: Some(ActionError {
                            code: 400,
                            message: err_msg,
                            detail: String::new(),
                            raw_output: None,
                        }),
                        plan_id: request.plan_id,
                        task_id: request.task_id,
                        correlation_id: request.correlation_id,
                    };
                    let json = serde_json::to_string(&response)
                        .unwrap_or_else(|_| r#"{"status":"error","code":400}"#.to_string());
                    return Err(json);
                }
            };

            let params_buf: Vec<String> = validated
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let params: Vec<&str> = params_buf.iter().map(|s| s.as_str()).collect();
            execute_git_command(repo_root, "diff", &params)
        }
        "git_log" => {
            let limit = request
                .payload
                .0
                .get("limit")
                .and_then(Value::as_str)
                .unwrap_or("10");
            execute_git_command(repo_root, "log", &["-n", limit])
        }
        "git_add" => {
            let files: Vec<String> = match request.payload.0.get("files") {
                Some(Value::Array(arr)) => arr
                    .iter()
                    .map(|v| v.as_str().unwrap_or("").to_string())
                    .collect(),
                _ => vec![],
            };

            if files.is_empty() {
                let msg = "No files specified for git add".to_string();
                let result = ActionResult {
                    output_type: "error".to_string(),
                    data: msg.clone(),
                    metadata: None,
                };
                let response = ActionResponse {
                    request_id: request.request_id,
                    api_version: request.api_version,
                    status: "error".to_string(),
                    code: 400,
                    result: Some(result),
                    error: Some(ActionError {
                        code: 400,
                        message: msg,
                        detail: String::new(),
                        raw_output: None,
                    }),
                    plan_id: request.plan_id,
                    task_id: request.task_id,
                    correlation_id: request.correlation_id,
                };
                let json = serde_json::to_string(&response)
                    .unwrap_or_else(|_| r#"{"status":"error","code":400}"#.to_string());
                return Err(json);
            }

            let validated = match validate_repo_paths(repo_root, &files) {
                Ok(paths) => paths,
                Err(err_msg) => {
                    let result = ActionResult {
                        output_type: "error".to_string(),
                        data: err_msg.clone(),
                        metadata: None,
                    };
                    let response = ActionResponse {
                        request_id: request.request_id,
                        api_version: request.api_version,
                        status: "error".to_string(),
                        code: 400,
                        result: Some(result),
                        error: Some(ActionError {
                            code: 400,
                            message: err_msg,
                            detail: String::new(),
                            raw_output: None,
                        }),
                        plan_id: request.plan_id,
                        task_id: request.task_id,
                        correlation_id: request.correlation_id,
                    };
                    let json = serde_json::to_string(&response)
                        .unwrap_or_else(|_| r#"{"status":"error","code":400}"#.to_string());
                    return Err(json);
                }
            };

            let params_buf: Vec<String> = validated
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let params: Vec<&str> = params_buf.iter().map(|s| s.as_str()).collect();
            execute_git_command(repo_root, "add", &params)
        }
        "git_commit" => {
            let message = request.payload.0.get("message").and_then(Value::as_str);
            match message {
                Some(msg) => execute_git_command(repo_root, "commit", &["-m", msg]),
                None => {
                    let msg = "Commit message not provided".to_string();
                    let result = ActionResult {
                        output_type: "error".to_string(),
                        data: msg.clone(),
                        metadata: None,
                    };
                    let response = ActionResponse {
                        request_id: request.request_id,
                        api_version: request.api_version,
                        status: "error".to_string(),
                        code: 400,
                        result: Some(result),
                        error: Some(ActionError {
                            code: 400,
                            message: msg,
                            detail: String::new(),
                            raw_output: None,
                        }),
                        plan_id: request.plan_id,
                        task_id: request.task_id,
                        correlation_id: request.correlation_id,
                    };
                    let json = serde_json::to_string(&response)
                        .unwrap_or_else(|_| r#"{"status":"error","code":400}"#.to_string());
                    return Err(json);
                }
            }
        }
        _ => {
            let msg = format!("Unknown action: {}", request.action);
            let result = ActionResult {
                output_type: "error".to_string(),
                data: msg.clone(),
                metadata: None,
            };
            let response = ActionResponse {
                request_id: request.request_id,
                api_version: request.api_version,
                status: "error".to_string(),
                code: 501,
                result: Some(result),
                error: Some(ActionError {
                    code: 501,
                    message: msg,
                    detail: String::new(),
                    raw_output: None,
                }),
                plan_id: request.plan_id,
                task_id: request.task_id,
                correlation_id: request.correlation_id,
            };
            let json = serde_json::to_string(&response)
                .unwrap_or_else(|_| r#"{"status":"error","code":501}"#.to_string());
            return Err(json);
        }
    };

    let response = ActionResponse {
        request_id: request.request_id,
        api_version: request.api_version,
        status: if result.output_type == "error" {
            "error".to_string()
        } else {
            "success".to_string()
        },
        code: if result.output_type == "error" { 1 } else { 0 },
        result: Some(result),
        error: None, // Error details are currently in result.data
        plan_id: request.plan_id,
        task_id: request.task_id,
        correlation_id: request.correlation_id,
    };

    Ok(response)
}

fn execute_git_command(repo_root: &Path, command: &str, args: &[&str]) -> ActionResult {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_root).arg(command).args(args);

    let output = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
        Ok(output) => output,
        Err(e) => {
            return ActionResult {
                output_type: "error".to_string(),
                data: format!("Failed to execute git {}: {}", command, e),
                metadata: None,
            };
        }
    };

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        ActionResult {
            output_type: "text".to_string(),
            data: stdout,
            metadata: None,
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        ActionResult {
            output_type: "error".to_string(),
            data: stderr,
            metadata: Some(serde_json::json!({
                "exit_code": output.status.code()
            })),
        }
    }
}
