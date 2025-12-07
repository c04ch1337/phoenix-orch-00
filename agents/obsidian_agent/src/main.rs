use serde_json::Value;
use shared_types::{ActionRequest, ActionResponse, ActionResult};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

mod safe_paths;
use crate::safe_paths::{load_vault_root_from_env, resolve_vault_path};

fn main() {
    platform::init_tracing("obsidian_agent").expect("failed to init tracing");

    // Resolve and validate Obsidian vault root from environment. If this fails,
    // we cannot safely operate and will exit with a non-zero status so the
    // orchestrator treats this as an agent startup failure.
    let vault_root = match load_vault_root_from_env() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[FATAL] {err}");
            return;
        }
    };

    let mut buffer = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buffer) {
        eprintln!("Failed to read from stdin: {}", e);
        return;
    }

    let request: ActionRequest = match serde_json::from_str(&buffer) {
        Ok(req) => req,
        Err(e) => {
            let response = ActionResponse {
                request_id: uuid::Uuid::new_v4(),
                api_version: None,
                status: "error".to_string(),
                code: 1,
                result: None,
                error: Some(format!("Failed to parse request: {}", e)),
                plan_id: None,
                task_id: None,
                correlation_id: None,
            };
            println!("{}", serde_json::to_string(&response).unwrap());
            return;
        }
    };

    let response = handle_request(&vault_root, request);

    let response_json = match serde_json::to_string(&response) {
        Ok(json) => json,
        Err(e) => {
            // Fallback error response if serialization fails
            format!(
                r#"{{"request_id":"{}","status":"error","code":1,"result":null,"error":"Failed to serialize response: {}"}}"#,
                response.request_id,
                e.to_string().replace("\"", "\\\"")
            )
        }
    };

    print!("{}", response_json);
}

fn handle_request(vault_root: &Path, request: ActionRequest) -> ActionResponse {
    let vault_path_val = match request.payload.0.get("vault_path") {
        Some(path) => path,
        None => {
            return ActionResponse {
                request_id: request.request_id,
                api_version: None,
                status: "error".to_string(),
                code: 400,
                result: Some(ActionResult {
                    output_type: "error".to_string(),
                    data: "Missing 'vault_path' in parameters".to_string(),
                    metadata: None,
                }),
                error: Some("Missing 'vault_path' in parameters".to_string()),
                plan_id: request.plan_id,
                task_id: request.task_id,
                correlation_id: request.correlation_id,
            };
        }
    };

    let vault_rel = match vault_path_val.as_str() {
        Some(path) => path,
        None => {
            return ActionResponse {
                request_id: request.request_id,
                api_version: None,
                status: "error".to_string(),
                code: 400,
                result: Some(ActionResult {
                    output_type: "error".to_string(),
                    data: "'vault_path' must be a string".to_string(),
                    metadata: None,
                }),
                error: Some("'vault_path' must be a string".to_string()),
                plan_id: request.plan_id,
                task_id: request.task_id,
                correlation_id: request.correlation_id,
            };
        }
    };

    // Resolve the vault-relative path safely under the configured root.
    let vault_path: PathBuf = match resolve_vault_path(vault_root, vault_rel) {
        Ok(p) => p,
        Err(msg) => {
            return ActionResponse {
                request_id: request.request_id,
                api_version: None,
                status: "error".to_string(),
                code: 400,
                result: Some(ActionResult {
                    output_type: "error".to_string(),
                    data: msg.clone(),
                    metadata: None,
                }),
                error: Some(msg),
                plan_id: request.plan_id,
                task_id: request.task_id,
                correlation_id: request.correlation_id,
            };
        }
    };

    let result = match request.action.as_str() {
        "create_note" => create_note(&request.payload.0, &vault_path),
        "read_note" => read_note(&request.payload.0, &vault_path),
        "update_note" => update_note(&request.payload.0, &vault_path),
        _ => Err(format!("Unknown action: {}", request.action)),
    };

    match result {
        Ok(action_result) => ActionResponse {
            request_id: request.request_id,
            api_version: None,
            status: "success".to_string(),
            code: 0,
            result: Some(action_result),
            error: None,
            plan_id: request.plan_id,
            task_id: request.task_id,
            correlation_id: request.correlation_id,
        },
        Err(e) => ActionResponse {
            request_id: request.request_id,
            api_version: None,
            status: "error".to_string(),
            code: 3,
            result: None,
            error: Some(e),
            plan_id: request.plan_id,
            task_id: request.task_id,
            correlation_id: request.correlation_id,
        },
    }
}

fn create_note(parameters: &Value, vault_path: &PathBuf) -> Result<ActionResult, String> {
    let note_name = parameters["note_name"]
        .as_str()
        .ok_or("Missing 'note_name'")?;
    let content = parameters["content"].as_str().ok_or("Missing 'content'")?;
    let path = vault_path.join(format!("{}.md", note_name));

    fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(ActionResult {
        output_type: "text".to_string(),
        data: format!("Note '{}' created successfully.", note_name),
        metadata: None,
    })
}

fn read_note(parameters: &Value, vault_path: &PathBuf) -> Result<ActionResult, String> {
    let note_name = parameters["note_name"]
        .as_str()
        .ok_or("Missing 'note_name'")?;
    let path = vault_path.join(format!("{}.md", note_name));

    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

    Ok(ActionResult {
        output_type: "text".to_string(),
        data: content,
        metadata: None,
    })
}

fn update_note(parameters: &Value, vault_path: &PathBuf) -> Result<ActionResult, String> {
    let note_name = parameters["note_name"]
        .as_str()
        .ok_or("Missing 'note_name'")?;
    let content = parameters["content"].as_str().ok_or("Missing 'content'")?;
    let path = vault_path.join(format!("{}.md", note_name));

    if !path.exists() {
        return Err(format!("Note '{}' not found.", note_name));
    }

    fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(ActionResult {
        output_type: "text".to_string(),
        data: format!("Note '{}' updated successfully.", note_name),
        metadata: None,
    })
}
