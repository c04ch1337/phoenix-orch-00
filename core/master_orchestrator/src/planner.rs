use crate::consciousness::MultilayerConsciousness;
use crate::executor::{execute_agent, execute_agent_for_task};
use crate::memory_service::MemoryService;
use crate::tool_service::ToolService;
use platform::{correlation_span, record_counter};
use serde_json::json;
use shared_types::{
    ActionRequest, ActionResponse, ActionResult, AgentHealthState, AppConfig, CorrelationId,
    OrchestratorError, OrchestratorErrorCode, Payload, PlanId, PlanStatus, TaskId, TaskStatus,
    EthicalRecommendation,
};
use std::sync::Arc;
use uuid::Uuid;

/// Output contract for the v1 planning/execution entrypoint.
pub struct PlanAndExecuteOutputV1 {
    pub plan_id: PlanId,
    pub root_task_id: TaskId,
    pub user_facing_output: String,
}

/// Error contract for the v1 planning/execution entrypoint.
pub struct PlanAndExecuteErrorV1 {
    pub correlation_id: CorrelationId,
    pub plan_id: Option<PlanId>,
    pub error: OrchestratorError,
}

fn agent_in_circuit_cooldown(
    health: AgentHealthState,
    circuit_open_until: &Option<String>,
) -> bool {
    if health != AgentHealthState::Unhealthy {
        return false;
    }

    if let Some(until) = circuit_open_until {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) {
            return dt > chrono::Utc::now();
        }
    }

    false
}

/// Simple heuristic to estimate token count (approx 4 chars per token).
/// This is not perfect but sufficient for a rough guardrail.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Truncates the context string to fit within the available token budget.
/// Returns the truncated context and a boolean indicating if truncation occurred.
fn truncate_context_to_fit(
    context: &str,
    user_message: &str,
    max_input_tokens: u32,
) -> (String, bool) {
    let user_tokens = estimate_tokens(user_message);
    let safety_margin = 100; // Buffer for system prompts, etc.
    
    if user_tokens + safety_margin >= max_input_tokens as usize {
        // User message alone is too big or close to limit. 
        // We can't do much about the user message here without altering intent,
        // so we just return empty context.
        return (String::new(), !context.is_empty());
    }

    let available_tokens = max_input_tokens as usize - user_tokens - safety_margin;
    let context_tokens = estimate_tokens(context);

    if context_tokens <= available_tokens {
        (context.to_string(), false)
    } else {
        // Simple character-based truncation
        let max_chars = available_tokens * 4;
        let truncated = if context.len() > max_chars {
             // Try to keep the *end* of the context if it's a list, or *beginning*?
             // Usually for RAG, the top hits are most relevant. 
             // But if it's chat history, recent is better.
             // Given the current simple implementation (appending lists), 
             // we'll just take the first N characters for now to ensure we include *some* top results.
             // A better strategy might be to filter the list of facts/memories *before* joining.
             // For now, simple char truncation.
             let mut s = context[..max_chars].to_string();
             s.push_str("\n...[truncated due to length]...");
             s
        } else {
            context.to_string()
        };
        (truncated, true)
    }
}


/// New v1 planning + execution entrypoint that wires in plan/task lifecycle tracking.
/// Now includes consciousness evaluation for ethical decision-making and prompt injection.
pub async fn plan_and_execute_v1(
    correlation_id: CorrelationId,
    user_message: String,
    context: Option<String>,
    memory_service: Arc<MemoryService>,
    app_config: Arc<AppConfig>,
    tool_service: Arc<ToolService>,
    consciousness: Arc<MultilayerConsciousness>,
) -> Result<PlanAndExecuteOutputV1, PlanAndExecuteErrorV1> {
    let span = correlation_span(correlation_id, "plan_and_execute_v1");
    let _enter = span.enter();
    record_counter("orchestrator_plan_started_total", 1);

    let plan_id = PlanId::new_v4();

    // Initial draft state with the raw user message as description.
    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Draft,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    // 1. Check Agent Registry
    let active_agents =
        memory_service
            .get_active_agents()
            .await
            .map_err(|e| PlanAndExecuteErrorV1 {
                correlation_id,
                plan_id: Some(plan_id),
                error: OrchestratorError {
                    code: OrchestratorErrorCode::PlanningFailed,
                    message: format!("Memory Error: {}", e),
                    details: None,
                },
            })?;

    // 2. Simple Intent Detection (Keyword-based for V1)
    let target_tool = if user_message.to_lowercase().contains("git")
        || user_message.to_lowercase().contains("commit")
    {
        "git_agent"
    } else {
        "llm_router_agent" // Fallback to the future LLM planning agent
    };

    // 3. Agent Validation Check (The Gatekeeper)
    if !active_agents.iter().any(|a| a.tool_name == target_tool) {
        let msg = format!(
            "Error: Agent '{}' is not registered or active.",
            target_tool
        );
        // Mark plan as failed before returning.
        let _ = memory_service
            .record_plan_state_change(plan_id, PlanStatus::Failed, Some(&msg), correlation_id)
            .await;
        record_counter("orchestrator_plan_failed_total", 1);

        return Err(PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::PlanningFailed,
                message: msg,
                details: None,
            },
        });
    }

    // 3b. Agent health check (Circuit Breaker).
    //
    // If the selected agent is marked as Unhealthy AND the circuit breaker is
    // still open (circuit_open_until is in the future), fail the plan early
    // with AgentUnavailable so that callers get a fast, clear failure.
    if let Ok(summary) = memory_service.get_agent_health(target_tool).await {
        if summary.health == AgentHealthState::Unhealthy {
            let mut in_cooldown = false;
            if let Some(ref until) = summary.circuit_open_until {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(until) {
                    if dt > chrono::Utc::now() {
                        in_cooldown = true;
                    }
                }
            }

            if in_cooldown {
                let msg = format!(
                    "Agent '{}' is temporarily unavailable due to recent failures.",
                    target_tool
                );

                let _ = memory_service
                    .record_plan_state_change(
                        plan_id,
                        PlanStatus::Failed,
                        Some(&msg),
                        correlation_id,
                    )
                    .await;
                record_counter("orchestrator_plan_failed_total", 1);

                return Err(PlanAndExecuteErrorV1 {
                    correlation_id,
                    plan_id: Some(plan_id),
                    error: OrchestratorError {
                        code: OrchestratorErrorCode::AgentUnavailable,
                        message: msg,
                        details: None,
                    },
                });
            }
        }
    }

    // --- Context Retrieval ---
    let mut context_str = String::new();

    // Include explicit request context if provided.
    if let Some(ref ctx) = context {
        if !ctx.is_empty() {
            context_str.push_str("\n[Request Context]:\n");
            context_str.push_str(ctx);
            context_str.push('\n');
        }
    }

    // A. Structured Retrieval (Simple keyword match for now)
    if let Ok(facts) = memory_service
        .retrieve_structured_context(&user_message)
        .await
    {
        if !facts.is_empty() {
            context_str.push_str("\n[Structured Memory]:\n");
            for fact in facts {
                context_str.push_str(&format!("- {}\n", fact));
            }
        }
    }

    // B. Semantic Retrieval
    if let Ok(memories) = memory_service
        .retrieve_semantic_context(&user_message, 3)
        .await
    {
        if !memories.is_empty() {
            context_str.push_str("\n[Semantic Memory]:\n");
            for mem in memories {
                context_str.push_str(&format!("- {}\n", mem));
            }
        }
    }

    // 4. Prepare Payload
    let mut payload_json = json!({ "prompt": user_message });

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
            let max_input_tokens = config.max_input_tokens.unwrap_or(128_000);
            
            // ========================================
            // CONSCIOUSNESS INTEGRATION
            // ========================================
            // Synthesize a conscious decision about the user's request
            let conscious_decision = consciousness.synthesize_decision(&user_message).await;
            
            // Log consciousness analysis
            tracing::info!(
                "Consciousness Analysis: patterns={:?}, ethical={}, confidence={:.2}",
                conscious_decision.mind_analysis.patterns_matched,
                conscious_decision.ethical_evaluation.is_ethical,
                conscious_decision.final_confidence
            );
            
            // Check ethical evaluation - reject if ethical evaluation says Reject
            if conscious_decision.ethical_evaluation.recommendation == EthicalRecommendation::Reject {
                tracing::warn!(
                    "Consciousness REJECTED request: harm_score={:.2}, reason: potential ethical violation",
                    conscious_decision.ethical_evaluation.harm_score
                );
                return Err(PlanAndExecuteErrorV1 {
                    correlation_id,
                    plan_id: Some(plan_id),
                    error: OrchestratorError {
                        code: OrchestratorErrorCode::ExecutionFailed,
                        message: format!(
                            "Request declined by ethical evaluation: harm score {:.2} exceeds threshold. {}",
                            conscious_decision.ethical_evaluation.harm_score,
                            conscious_decision.synthesis_notes
                        ),
                        details: None,
                    },
                });
            }
            
            // Get consciousness prompts from environment
            let consciousness_default_prompt = std::env::var("CONSCIOUSNESS_DEFAULT_PROMPT")
                .unwrap_or_else(|_| "You are Phoenix, an AI assistant with world-class cybersecurity expertise in both Red Team (pentesting, social engineering, exploits, zero-day) and Blue Team (threat hunting, incident response, SIEM, automation).".to_string());
            
            let consciousness_master_prompt = std::env::var("CONSCIOUSNESS_MASTER_PROMPT")
                .unwrap_or_else(|_| "Phoenix operates with a strong ethical foundation, prioritizing human safety while maintaining world-class cybersecurity capabilities. Apply adversarial thinking when analyzing threats, and always recommend defense-in-depth strategies.".to_string());
            
            // Build the consciousness-enhanced system prompt
            let system_prompt = format!(
                "{}\n\n{}\n\n[Consciousness State: confidence={:.2}, reasoning={}]",
                consciousness_default_prompt,
                consciousness_master_prompt,
                conscious_decision.final_confidence,
                conscious_decision.mind_analysis.reasoning_approach
            );
            
            // Add professional context if applicable
            let professional_context = if let Some(ref assessment) = conscious_decision.professional_assessment {
                if assessment.expertise_applicable {
                    format!("\n[Professional Expertise Engaged: {}]", assessment.recommended_approach)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            
            // Add ethical guidance if caution is recommended
            let ethical_guidance = if conscious_decision.ethical_evaluation.recommendation == EthicalRecommendation::Caution {
                format!(
                    "\n[CAUTION: Proceeding with care - harm_score={:.2}, benefit_score={:.2}]",
                    conscious_decision.ethical_evaluation.harm_score,
                    conscious_decision.ethical_evaluation.benefit_score
                )
            } else {
                String::new()
            };
            
            let (final_context, truncated) = if !context_str.is_empty() {
                truncate_context_to_fit(&context_str, &user_message, max_input_tokens)
            } else {
                (String::new(), false)
            };

            if truncated {
                println!("Warning: Context was truncated to fit max_input_tokens={}", max_input_tokens);
            }

            // Build the consciousness-enhanced prompt
            let final_prompt = format!(
                "{system}\n{prof}{ethical}\n\n[User Request]:\n{user}\n\n{context}",
                system = system_prompt,
                prof = professional_context,
                ethical = ethical_guidance,
                user = user_message,
                context = if !final_context.is_empty() {
                    format!("[Context]:\n{}", final_context)
                } else {
                    String::new()
                }
            );

            payload_json = json!({
                "prompt": final_prompt,
                "config": {
                    "provider": default_provider,
                    "api_key": config.api_key,
                    "base_url": config.base_url,
                    "model_name": config.model_name
                }
            });
            
            tracing::info!("Consciousness-enhanced prompt prepared: {} chars", final_prompt.len());
        }
    }

    // 5. Create the root task and record lifecycle.
    let root_task_id = TaskId::new_v4();

    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Pending,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    memory_service
        .record_task_state_change(
            root_task_id,
            plan_id,
            TaskStatus::Queued,
            None,
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    memory_service
        .record_plan_state_change(
            plan_id,
            PlanStatus::Running,
            Some(&user_message),
            correlation_id,
        )
        .await
        .map_err(|e| PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::Internal,
                message: e,
                details: None,
            },
        })?;

    // 6. Create the ActionRequest (The Universal Contract)
    let mut request = ActionRequest {
        request_id: Uuid::new_v4(),
        api_version: None,
        tool: target_tool.to_string(),
        action: "execute".to_string(), // Default action for now, agents can parse args
        context: context_str,          // Also pass context in the dedicated field
        plan_id: None,
        task_id: None,
        correlation_id: Some(correlation_id),
        payload: Payload(payload_json),
    };

    // 7. Execute via the lifecycle-aware executor wrapper or use cached LLM router.
    let exec_result = if target_tool == "llm_router_agent" {
        // Use cached execution for LLM router agent via shared executor.

        // Extract Redis config if available
        let redis_config = app_config.redis.as_ref();

        // Update task status to InProgress
        memory_service
            .record_task_state_change(
                root_task_id,
                plan_id,
                TaskStatus::InProgress,
                None,
                correlation_id,
            )
            .await
            .map_err(|e| PlanAndExecuteErrorV1 {
                correlation_id,
                plan_id: Some(plan_id),
                error: OrchestratorError {
                    code: OrchestratorErrorCode::Internal,
                    message: e,
                    details: None,
                },
            })?;

        let timeout_duration = if let Some(agents_cfg) = &app_config.agents {
            let exec_cfg = agents_cfg
                .llm_router_agent
                .as_ref()
                .unwrap_or(&agents_cfg.default);
            std::time::Duration::from_secs(exec_cfg.timeout_secs)
        } else {
            // Legacy default when no agents config is provided.
            std::time::Duration::from_secs(30)
        };

        tool_service
            .execute_llm_router_with_caching(&request, redis_config, timeout_duration)
            .await
    } else {
        // Use standard agent executor for non-LLM agents
        execute_agent_for_task(
            target_tool,
            plan_id,
            root_task_id,
            correlation_id,
            &mut request,
            memory_service.as_ref(),
            app_config.as_ref(),
        )
        .await
    };

    let response = exec_result;

    if response.status == "success" {
        // Mark plan as succeeded.
        memory_service
            .record_plan_state_change(
                plan_id,
                PlanStatus::Succeeded,
                Some(&user_message),
                correlation_id,
            )
            .await
            .map_err(|e| PlanAndExecuteErrorV1 {
                correlation_id,
                plan_id: Some(plan_id),
                error: OrchestratorError {
                    code: OrchestratorErrorCode::Internal,
                    message: e,
                    details: None,
                },
            })?;
        record_counter("orchestrator_plan_succeeded_total", 1);

        // Log action trace (This will now also index the action semantically!)
        if let Err(e) = memory_service.log_action_trace(&request, &response).await {
            eprintln!("Failed to log action trace: {}", e);
        }

        let user_facing_output = serde_json::to_string(&response.result).unwrap_or_default();

        Ok(PlanAndExecuteOutputV1 {
            plan_id,
            root_task_id,
            user_facing_output,
        })
    } else {
        // Treat any non-success status as a failed plan.
        let error_message = if let Some(err) = &response.error {
            // Prefer structured error message when available.
            err.message.clone()
        } else {
            format!("Execution failed with code {}", response.code)
        };

        let _ = memory_service
            .record_plan_state_change(
                plan_id,
                PlanStatus::Failed,
                Some(&format!("Execution failed: {}", error_message)),
                correlation_id,
            )
            .await;

        record_counter("orchestrator_plan_failed_total", 1);

        Err(PlanAndExecuteErrorV1 {
            correlation_id,
            plan_id: Some(plan_id),
            error: OrchestratorError {
                code: OrchestratorErrorCode::ExecutionFailed,
                message: error_message,
                details: None,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn agent_in_circuit_cooldown_returns_false_for_non_unhealthy_or_missing_deadline() {
        // Healthy agents should never be considered in cooldown.
        assert!(!agent_in_circuit_cooldown(AgentHealthState::Healthy, &None));

        // Unhealthy but without a circuit_open_until timestamp is treated as not in cooldown.
        assert!(!agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &None
        ));
    }

    #[test]
    fn agent_in_circuit_cooldown_checks_future_and_past_deadlines() {
        let future = (Utc::now() + Duration::minutes(5)).to_rfc3339();
        let past = (Utc::now() - Duration::minutes(5)).to_rfc3339();

        // Unhealthy with a future deadline -> in cooldown.
        assert!(agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &Some(future)
        ));

        // Unhealthy but deadline in the past -> no longer in cooldown.
        assert!(!agent_in_circuit_cooldown(
            AgentHealthState::Unhealthy,
            &Some(past)
        ));
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("1234"), 1);
        assert_eq!(estimate_tokens("12345678"), 2);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_truncate_context_to_fit() {
        let user_msg = "Hello"; // 5 chars -> 1 token
        let context = "This is a long context string that needs to be truncated."; // 57 chars -> 14 tokens
        
        // Case 1: Fits comfortably
        // Max 1000 tokens. User=1, Safety=100. Available=899. Context=14.
        let (truncated, was_truncated) = truncate_context_to_fit(context, user_msg, 1000);
        assert_eq!(truncated, context);
        assert!(!was_truncated);

        // Case 2: Needs truncation
        // Max 105 tokens. User=1, Safety=100. Available=4 tokens (16 chars).
        let (truncated, was_truncated) = truncate_context_to_fit(context, user_msg, 105);
        assert!(was_truncated);
        assert!(truncated.contains("...[truncated due to length]..."));
        assert!(truncated.len() < context.len());
        // 16 chars max + suffix
        assert_eq!(truncated, "This is a long c\n...[truncated due to length]...");

        // Case 3: User message too big
        // Max 100 tokens. User=1, Safety=100. Available < 0.
        let (truncated, was_truncated) = truncate_context_to_fit(context, user_msg, 100);
        assert!(was_truncated);
        assert_eq!(truncated, "");
    }
}
