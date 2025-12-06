use shared_types::{ActionRequest, ActionResponse, ActionResult};
use std::io::{self, Read};

fn main() {
    // 1. Read JSON ActionRequest from STDIN
    let mut buffer = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut buffer) {
        eprintln!("Failed to read from stdin: {}", e);
        return;
    }

    let request: ActionRequest = match serde_json::from_str(&buffer) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            return;
        }
    };

    // 2. Implement placeholder function execute_commit_and_push
    let result = if request.action == "commit_and_push" {
        execute_commit_and_push()
    } else {
        ActionResult {
            output_type: "error".to_string(),
            data: format!("Unknown action: {}", request.action),
            metadata: None,
        }
    };

    // 3. Generate and write JSON ActionResponse to STDOUT
    let response = ActionResponse {
        request_id: request.request_id,
        status: "success".to_string(),
        code: 0,
        result: Some(result),
        error: None,
    };

    let response_json = match serde_json::to_string(&response) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("Failed to serialize response: {}", e);
            return;
        }
    };

    print!("{}", response_json);
}

fn execute_commit_and_push() -> ActionResult {
    // Placeholder logic
    ActionResult {
        output_type: "text".to_string(),
        data: "Committed and pushed changes successfully (placeholder).".to_string(),
        metadata: None,
    }
}
