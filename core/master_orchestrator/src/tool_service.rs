use crate::cache_service;
// Add explicit re-export for library access
pub mod cache_service_reexport {
    
}
use crate::tool_registry_service;
use crate::executor::execute_agent;
use rusqlite::Connection;
use serde_json::{json, Value};
use shared_types::{ActionRequest, ActionResponse, ActionResult, RedisConfig, Tool, API_VERSION_CURRENT};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, warn};

const MAX_TOOL_ARG_BYTES: usize = 8 * 1024;

/// Service responsible for loading and invoking registered tools.
pub struct ToolService {
    pub tools: HashMap<String, Tool>,
}

impl ToolService {
    /// Create a new ToolService by loading tools from the registry database.
    pub fn new(conn: &Connection) -> Result<Self, String> {
        let tools = tool_registry_service::load_tools(conn).map_err(|e| e.to_string())?;

        let mut tool_map = HashMap::new();
        for tool in tools {
            tool_map.insert(tool.name.clone(), tool);
        }

        Ok(ToolService { tools: tool_map })
    }

    /// Execute the LLM router agent with caching support.
    ///
    /// This implementation:
    /// - Uses the shared executor to invoke the `llm_router_agent` binary directly.
    /// - Sends the full `ActionRequest` JSON over STDIN.
    /// - Keeps the command line small (no large JSON CLI args or env vars).
    /// - Caches successful text responses keyed by (provider, model, prompt).
    pub async fn execute_llm_router_with_caching(
        &self,
        request: &ActionRequest,
        redis_config: Option<&RedisConfig>,
        timeout_duration: Duration,
    ) -> ActionResponse {
        // Extract LLM router domain payload from the ActionRequest.
        let payload = &request.payload.0;
        let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
        let config = match payload.get("config").and_then(|v| v.as_object()) {
            Some(c) => c,
            None => {
                warn!(
                    "LLM router payload missing 'config' object; skipping cache and delegating to agent"
                );
                return execute_agent("llm_router_agent", request, timeout_duration).await;
            }
        };
    
        let provider = config
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let model = config
            .get("model_name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
    
        // Try to get from cache first.
        if let Some(cached) = cache_service::get_cached_llm_response(provider, model, prompt) {
            debug!(
                "Using cached LLM response for prompt hash: {}",
                cache_service::generate_llm_cache_key(provider, model, prompt)
            );
    
            let result = ActionResult {
                output_type: "text".to_string(),
                data: cached.content,
                metadata: Some(json!({
                    "provider": provider,
                    "model": model,
                    "cached": true,
                    "cached_at": cached.cached_at
                })),
            };
    
            return ActionResponse {
                request_id: request.request_id,
                api_version: request.api_version.or(Some(API_VERSION_CURRENT)),
                status: "success".to_string(),
                code: 0,
                result: Some(result),
                error: None,
                plan_id: request.plan_id,
                task_id: request.task_id,
                correlation_id: request.correlation_id,
            };
        }
    
        // No cache hit, execute the agent via the shared executor, which:
        // - Invokes the agent executable directly (no cmd.exe wrapper).
        // - Writes the serialized ActionRequest JSON to the child's STDIN.
        debug!("Cache miss for LLM prompt, calling llm_router_agent");
        let response = execute_agent("llm_router_agent", request, timeout_duration).await;
    
        // Cache only successful text responses.
        if response.status == "success" && response.code == 0 {
            if let Some(ref result) = response.result {
                if cache_service::cache_llm_response(
                    provider,
                    model,
                    prompt,
                    &result.data,
                    redis_config,
                ) {
                    debug!("Successfully cached LLM response");
                } else {
                    warn!("Failed to cache LLM response");
                }
            }
        }
    
        response
    }

    /// Execute a registered tool by spawning its executable with the given params.
    pub async fn execute_tool(&self, tool_name: &str, params: &[&str]) -> Result<String, String> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| "Tool not found".to_string())?;

        // Defensive guard against excessively long CLI argument vectors. This keeps
        // `execute_tool` safe for non-chat usage while encouraging stdin-based execution
        // for large payloads.
        let total_arg_bytes: usize = params.iter().map(|s| s.len()).sum();
        if total_arg_bytes > MAX_TOOL_ARG_BYTES {
            warn!(
                tool = tool_name,
                total_arg_bytes,
                max_allowed = MAX_TOOL_ARG_BYTES,
                "Refusing to execute tool with excessively long arguments; use stdin-based execution instead"
            );
            return Err(
                "Refusing to execute tool with excessively long arguments; use stdin-based execution instead."
                    .to_string(),
            );
        }

        let output = tokio::process::Command::new(&tool.executable_path)
            .args(params)
            .output()
            .await
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
