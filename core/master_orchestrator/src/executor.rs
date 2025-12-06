use shared_types::{ActionRequest, ActionResponse};
use std::process::{Command, Stdio};
use std::io::Write;

pub fn execute_agent(agent_name: &str, request: &ActionRequest) -> Result<ActionResponse, String> {
    // Assuming binaries are in target/debug for development
    // In a real scenario, this path would be configurable
    let binary_name = if cfg!(target_os = "windows") {
        format!("{}.exe", agent_name)
    } else {
        agent_name.to_string()
    };
    
    let binary_path = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join("target/debug")
        .join(&binary_name);

    let mut child = Command::new(&binary_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn agent {} at {:?}: {}", agent_name, binary_path, e))?;

    let request_json = serde_json::to_string(request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request_json.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Failed to wait on child: {}", e))?;

    if !output.status.success() {
        return Err(format!("Agent exited with non-zero status: {:?}", output.status));
    }

    let response: ActionResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to deserialize response: {}", e))?;

    Ok(response)
}
