use shared_types::{ActionRequest, ActionResponse, ActionResult};
use std::io::{self, Read};
use serde_json::json;
use reqwest::Client;

#[tokio::main]
async fn main() {
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

    // 2. Process Request
    let payload = &request.payload.0;
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let config = payload.get("config");

    let result = if let Some(config) = config {
        let api_key = config.get("api_key").and_then(|v| v.as_str()).unwrap_or("");
        let base_url = config.get("base_url").and_then(|v| v.as_str()).unwrap_or("https://openrouter.ai/api/v1");
        let model_name = config.get("model_name").and_then(|v| v.as_str()).unwrap_or("google/gemini-2.0-flash-exp:free");

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

async fn call_llm_provider(base_url: &str, api_key: &str, model: &str, prompt: &str) -> Result<String, String> {
    let client = Client::new();
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ]
    });

    let res = client.post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        // OpenRouter specific header
        .header("HTTP-Referer", "http://localhost:8181") 
        .header("X-Title", "Twin Orchestrator")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("API Error {}: {}", status, text));
    }

    let json: serde_json::Value = res.json().await
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("No content in response")?
        .to_string();

    Ok(content)
}
