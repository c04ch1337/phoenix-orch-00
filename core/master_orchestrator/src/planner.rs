use crate::memory_service::MemoryService;
use crate::executor::execute_agent;
use shared_types::{ActionRequest, ActionResponse, Payload, AppConfig};
use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

// Planner's main job: turn user intent into an ActionRequest
pub async fn plan_and_execute(
    user_message: String, 
    memory_service: Arc<MemoryService>,
    app_config: Arc<AppConfig>
) -> Result<ActionResponse, String> {
    // 1. Check Agent Registry
    let active_agents = memory_service.get_active_agents().await.map_err(|e| format!("Memory Error: {}", e))?;
    
    // 2. Simple Intent Detection (Keyword-based for V1)
    let target_tool = if user_message.to_lowercase().contains("git") || user_message.to_lowercase().contains("commit") {
        "git_agent" 
    } else {
        "llm_router_agent" // Fallback to the future LLM planning agent
    };
    
    // 3. Agent Validation Check (The Gatekeeper)
    if !active_agents.iter().any(|a| a.tool_name == target_tool) {
        return Err(format!("Error: Agent '{}' is not registered or active.", target_tool));
    }

    // --- NEW: Context Retrieval ---
    let mut context_str = String::new();
    
    // A. Structured Retrieval (Simple keyword match for now)
    // In a real system, we'd extract entities. Here we just try the whole message or keywords.
    if let Ok(facts) = memory_service.retrieve_structured_context(&user_message).await {
        if !facts.is_empty() {
            context_str.push_str("\n[Structured Memory]:\n");
            for fact in facts {
                context_str.push_str(&format!("- {}\n", fact));
            }
        }
    }

    // B. Semantic Retrieval
    if let Ok(memories) = memory_service.retrieve_semantic_context(&user_message, 3).await {
        if !memories.is_empty() {
            context_str.push_str("\n[Semantic Memory]:\n");
            for mem in memories {
                context_str.push_str(&format!("- {}\n", mem));
            }
        }
    }

    // 4. Prepare Payload
    let mut payload_json = json!({"prompt": user_message});

    // Inject LLM Config if target is llm_router_agent
    if target_tool == "llm_router_agent" {
        let default_provider = &app_config.llm.default_provider;
        let provider_config = match default_provider.as_str() {
            "openrouter" => &app_config.llm.openrouter,
            "gemini" => &app_config.llm.gemini,
            "grok" => &app_config.llm.grok,
            "openai" => &app_config.llm.openai,
            "anthropic" => &app_config.llm.anthropic,
            "ollama" => &app_config.llm.ollama,
            "lmstudio" => &app_config.llm.lmstudio,
            _ => &None,
        };

        if let Some(config) = provider_config {
            // Append context to the prompt for the LLM
            let final_prompt = if !context_str.is_empty() {
                format!("{}\n\nContext:\n{}", user_message, context_str)
            } else {
                user_message.clone()
            };

            payload_json = json!({
                "prompt": final_prompt,
                "config": {
                    "provider": default_provider,
                    "api_key": config.api_key,
                    "base_url": config.base_url,
                    "model_name": config.model_name
                }
            });
        }
    }
    
    // 5. Create the ActionRequest (The Universal Contract)
    let request = ActionRequest {
        request_id: Uuid::new_v4(),
        tool: target_tool.to_string(),
        action: "execute".to_string(), // Default action for now, agents can parse args
        context: context_str, // Also pass context in the dedicated field
        payload: Payload(payload_json),
    };

    // 6. Execute the Agent
    let response = execute_agent(target_tool, &request)?;
    
    // Log action trace (This will now also index the action semantically!)
    if let Err(e) = memory_service.log_action_trace(&request, &response).await {
        eprintln!("Failed to log action trace: {}", e);
    }

    Ok(response)
}
