use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct Payload(pub Value);

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionRequest {
    pub request_id: Uuid,
    pub tool: String,
    pub action: String,
    pub context: String,
    pub payload: Payload,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionResult {
    pub output_type: String,
    pub data: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ActionResponse {
    pub request_id: Uuid,
    pub status: String,
    pub code: u16,
    pub result: Option<ActionResult>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LLMConfig {
    pub default_provider: String,
    pub openrouter: Option<ProviderConfig>,
    pub gemini: Option<ProviderConfig>,
    pub grok: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub anthropic: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
    pub lmstudio: Option<ProviderConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub llm: LLMConfig,
}
