use once_cell::sync::Lazy;
use reqwest::Client;
use serde_json::json;
use shared_types::{ActionRequest, ActionResponse, ActionResult};
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::time::Duration;

fn log_to_file(msg: &str) {
    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open("agent_debug.log")
        .and_then(|mut file| writeln!(file, "{}", msg));
}

static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
});

#[tokio::main]
async fn main() {
    platform::init_tracing("llm_router_agent").expect("failed to init tracing");
    // 1. Read JSON ActionRequest from STDIN (single line)
    log_to_file("Agent started. Reading stdin line...");
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut buffer = String::new();
    
    match reader.read_line(&mut buffer) {
        Ok(0) => {
            log_to_file("EOF received with no data");
            eprintln!("EOF received with no data");
            return;
        }
        Ok(_) => {
            log_to_file(&format!("Received {} bytes", buffer.len()));
        }
        Err(e) => {
            log_to_file(&format!("Failed to read from stdin: {}", e));
            eprintln!("Failed to read from stdin: {}", e);
            return;
        }
    }
    eprintln!("[LLM Agent] Received request of size: {}", buffer.len());
    log_to_file(&format!("Received request of size: {}", buffer.len()));
    eprintln!("[LLM Agent] Received request of size: {}", buffer.len());

    let request: ActionRequest = match serde_json::from_str(&buffer) {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to parse request: {}", e);
            return;
        }
    };

    // 2. Process Request
    let payload = &request.payload.0;
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let config = payload.get("config");

    let result = if let Some(config) = config {
        let api_key = config.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
        let base_url = config
            .get("base_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://openrouter.ai/api/v1");
        let model_name = config
            .get("model_name")
            .and_then(|v| v.as_str())
            .unwrap_or("google/gemini-2.0-flash-exp:free");

        log_to_file(&format!("Calling provider: {}, model: {}", base_url, model_name));
        eprintln!("[LLM Agent] Calling provider: {}, model: {}", base_url, model_name);

        match call_llm_provider(base_url, api_key, model_name, prompt).await {
            Ok(response_text) => ActionResult {
                output_type: "text".to_string(),
                data: response_text,
                metadata: Some(json!({
                    "provider": config.get("provider"),
                    "model": model_name
                })),
            },
            Err(e) => ActionResult {
                output_type: "error".to_string(),
                data: format!("LLM Call Failed: {}", e),
                metadata: None,
            },
        }
    } else {
        ActionResult {
            output_type: "error".to_string(),
            data: "No LLM configuration provided in payload.".to_string(),
            metadata: None,
        }
    };

    // 3. Generate and write JSON ActionResponse to STDOUT
    let response = ActionResponse {
        request_id: request.request_id,
        api_version: None,
        status: "success".to_string(),
        code: 0,
        result: Some(result),
        error: None,
        plan_id: request.plan_id,
        task_id: request.task_id,
        correlation_id: request.correlation_id,
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

async fn call_llm_provider(
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, String> {
    // Use a shared HTTP client with a sane timeout
    let client: &Client = &*HTTP_CLIENT;

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ]
    });

    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        // OpenRouter specific headers (fine for production)
        .header("HTTP-Referer", "http://localhost:8282")
        .header("X-Title", "Twin Orchestrator")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        log_to_file(&format!("API Error: Status={}, Body={}", status, text));
        eprintln!("[LLM Agent] API Error: Status={}, Body={}", status, text);
        return Err(format!("API Error {}: {}", status, text));
    }

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| "No content in response".to_string())?
        .to_string();

    Ok(content)
}
