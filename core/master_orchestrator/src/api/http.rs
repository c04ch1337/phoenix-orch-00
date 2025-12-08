use std::sync::Arc;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use serde::Serialize;

use super::ApiContext;
use crate::memory_service::MemoryService;
use crate::planner;
use crate::tool_service::ToolService;
use platform::{correlation_span, extract_correlation_id, record_counter};
use shared_types::{AppConfig, ChatRequestV1, ChatResponseV1, CorrelationId, API_VERSION_CURRENT};
use tracing::{error, info, Instrument};


/// Simple health response used by `/health`.
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub llm_provider: String,
    pub llm_model: String,
}

pub fn configure(cfg: &mut web::ServiceConfig, ctx: ApiContext) {
    let ctx_data = web::Data::new(ctx);

    cfg.app_data(ctx_data.clone())
        .route("/api/v1/chat", web::post().to(chat_v1))
        .route("/api/v1/agents", web::get().to(list_agents))
        .route("/api/v1/consciousness", web::get().to(consciousness_state))
        .route("/api/v1/consciousness/learn", web::post().to(consciousness_learn))
        .route("/health", web::get().to(health));
}

// CORS middleware to allow cross-origin requests
use actix_web::http::header;
use actix_cors::Cors;

pub fn configure_cors(cfg: &mut web::ServiceConfig) {
    let cors = Cors::default()
        .allowed_origin("http://localhost:8282")
        .allowed_origin("http://127.0.0.1:8282")
        .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
        .allowed_headers(vec![
            header::AUTHORIZATION,
            header::ACCEPT,
            header::CONTENT_TYPE,
        ])
        .supports_credentials()
        .max_age(3600);

    cfg.service(
        web::scope("")
            .wrap(cors)
    );
}

use super::auth::verify_auth;

/// JWT-based authentication middleware.
///
/// If `ctx.jwt_auth` is `None`, authentication is disabled and all
/// requests are allowed. Otherwise, this verifies the JWT token in the
/// Authorization header. On failure, a `401 Unauthorized` response is returned.
pub async fn require_auth(req: &HttpRequest, ctx: &ApiContext) -> Result<(), HttpResponse> {
    if let Some(jwt_auth) = &ctx.jwt_auth {
        match verify_auth(req, jwt_auth).await {
            Ok(_) => Ok(()),
            Err(_) => Err(HttpResponse::Unauthorized().finish())
        }
    } else {
        Ok(())
    }
}


// New versioned /api/v1/chat endpoint using shared ChatRequestV1/ChatResponseV1 contracts.
async fn chat_v1(
    req: HttpRequest,
    body: web::Json<ChatRequestV1>,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    // Use existing correlation ID if provided, or create a new one
    let correlation_id = extract_correlation_id(body.correlation_id);
    let span = correlation_span(correlation_id, "chat_v1");
    
    // Execute within the correlation span
    async move {
        if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
            return Ok(resp);
        }
        record_counter("http_requests_total_chat_v1", 1);
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/chat",
            message_length = body.message.len(),
            has_context = body.context.is_some(),
            "Received v1 chat request"
        );
        
        handle_chat(body.into_inner(), ctx, correlation_id).await
    }.instrument(span).await
}

async fn handle_chat(
    mut req: ChatRequestV1,
    ctx: web::Data<ApiContext>,
    correlation_id: CorrelationId,
) -> Result<HttpResponse, Error> {
    // Ensure correlation ID is set in the request
    req.correlation_id = Some(correlation_id);
    
    // Create a span for this operation
    let span = correlation_span(correlation_id, "handle_chat");
    
    // Execute within the correlation span
    async move {
        let result = planner::plan_and_execute_v1(
            correlation_id,
            req.message.clone(),
            req.context.clone(),
            ctx.memory_service.clone(),
            ctx.app_config.clone(),
            ctx.tool_service.clone(),
            ctx.consciousness.clone(),
        )
        .await;

        match result {
            Ok(out) => {
                info!(
                    correlation_id = %correlation_id,
                    endpoint = "/api/v1/chat",
                    status = "success",
                    plan_id = %out.plan_id,
                    "Chat v1 request succeeded"
                );
                
                let resp = ChatResponseV1 {
                    api_version: API_VERSION_CURRENT,
                    correlation_id,
                    status: "success".to_string(),
                    plan_id: Some(out.plan_id),
                    output: Some(out.user_facing_output),
                    error: None,
                };
                Ok(HttpResponse::Ok().json(resp))
            }
            Err(e) => {
                error!(
                    correlation_id = %correlation_id,
                    endpoint = "/api/v1/chat",
                    status = "error",
                    plan_id = ?e.plan_id,
                    error_code = ?e.error.code,
                    error_message = %e.error.message,
                    "Chat v1 request failed"
                );
                
                let resp = ChatResponseV1 {
                    api_version: API_VERSION_CURRENT,
                    correlation_id,
                    status: "error".to_string(),
                    plan_id: e.plan_id,
                    output: None,
                    error: Some(e.error),
                };
                Ok(HttpResponse::Ok().json(resp))
            }
        }
    }.instrument(span).await
}

async fn list_agents(req: HttpRequest, ctx: web::Data<ApiContext>) -> Result<HttpResponse, Error> {
    // Generate correlation ID for this request
    let correlation_id = extract_correlation_id(None);
    let span = correlation_span(correlation_id, "list_agents");
    
    // Execute within the correlation span
    async move {
        if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
            return Ok(resp);
        }
        record_counter("http_requests_total_agents_v1", 1);
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/agents",
            "Retrieving agent health"
        );

        match ctx.memory_service.list_agent_health().await {
            Ok(summaries) => {
                info!(
                    correlation_id = %correlation_id,
                    endpoint = "/api/v1/agents",
                    agent_count = summaries.len(),
                    "Agent health retrieved successfully"
                );
                Ok(HttpResponse::Ok().json(summaries))
            },
            Err(e) => {
                error!(
                    correlation_id = %correlation_id,
                    endpoint = "/api/v1/agents",
                    error = %e,
                    "Failed to retrieve agent health"
                );
                Err(actix_web::error::ErrorInternalServerError(e))
            }
        }
    }.instrument(span).await
}

async fn health(req: HttpRequest, ctx: web::Data<ApiContext>) -> Result<HttpResponse, Error> {
    // Generate correlation ID for this request
    let correlation_id = extract_correlation_id(None);
    let span = correlation_span(correlation_id, "health");
    
    // Execute within the correlation span
    async move {
        if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
            return Ok(resp);
        }
        record_counter("http_requests_total_health", 1);
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/health",
            "Health check initiated"
        );

        let app_config: Arc<AppConfig> = ctx.app_config.clone();

        let provider = app_config.llm.default_provider.clone();
        let model = match provider.as_str() {
            "openrouter" => app_config
                .llm
                .openrouter
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "gemini" => app_config
                .llm
                .gemini
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "grok" => app_config
                .llm
                .grok
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "openai" => app_config
                .llm
                .openai
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "anthropic" => app_config
                .llm
                .anthropic
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "ollama" => app_config
                .llm
                .ollama
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            "lmstudio" => app_config
                .llm
                .lmstudio
                .as_ref()
                .map(|c| c.model_name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            _ => "unknown".to_string(),
        };

        let provider_clone = provider.clone();
        let model_clone = model.clone();
        
        let health = HealthResponse {
            status: "ok",
            llm_provider: provider,
            llm_model: model,
        };

        info!(
            correlation_id = %correlation_id,
            endpoint = "/health",
            llm_provider = %provider_clone,
            llm_model = %model_clone,
            "Health check completed successfully"
        );

        Ok(HttpResponse::Ok().json(health))
    }.instrument(span).await
}

// ===========================================
// CONSCIOUSNESS API ENDPOINTS
// ===========================================

/// Response structure for consciousness state endpoint
#[derive(Serialize)]
pub struct ConsciousnessStateResponse {
    pub status: String,
    pub active_layers: Vec<String>,
    pub mind: MindLayerState,
    pub heart: HeartLayerState,
    pub work: WorkLayerState,
    pub soul: SoulLayerState,
    pub social: SocialLayerState,
    pub body: BodyLayerState,
    pub creative: CreativeLayerState,
    pub overall_coherence: f32,
}

#[derive(Serialize)]
pub struct MindLayerState {
    pub focus_level: f32,
    pub mental_energy: f32,
    pub active_reasoning_model: String,
    pub patterns_in_memory: usize,
}

#[derive(Serialize)]
pub struct HeartLayerState {
    pub compassion_level: f32,
    pub ethical_strictness: String,
    pub harm_threshold: f32,
    pub values_count: usize,
}

#[derive(Serialize)]
pub struct WorkLayerState {
    pub initialized: bool,
    pub red_team_skills: usize,
    pub blue_team_skills: usize,
    pub tool_proficiency: usize,
    pub lessons_learned: usize,
}

#[derive(Serialize)]
pub struct SoulLayerState {
    pub purpose_clarity: f32,
    pub fulfillment_level: f32,
    pub core_mission: String,
    pub beliefs_count: usize,
}

#[derive(Serialize)]
pub struct SocialLayerState {
    pub relationships_count: usize,
    pub total_interactions: u64,
    pub social_effectiveness: f32,
    pub empathy_level: f32,
}

#[derive(Serialize)]
pub struct BodyLayerState {
    pub overall_health: f32,
    pub processing_capacity: f32,
    pub energy_status: String,
    pub tasks_processed: u64,
}

#[derive(Serialize)]
pub struct CreativeLayerState {
    pub creative_flow: f32,
    pub innovations_count: usize,
    pub insights_count: usize,
    pub creative_readiness: f32,
}

/// GET /api/v1/consciousness - Get current consciousness state
async fn consciousness_state(
    req: HttpRequest,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    let correlation_id = extract_correlation_id(None);
    let span = correlation_span(correlation_id, "consciousness_state");
    
    async move {
        if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
            return Ok(resp);
        }
        record_counter("http_requests_total_consciousness", 1);
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/consciousness",
            "Retrieving consciousness state"
        );
        
        let consciousness = &ctx.consciousness;
        
        // Get state from each layer
        let mind = consciousness.mind_kb.read().await;
        let heart = consciousness.heart_kb.read().await;
        let work = consciousness.work_kb.read().await;
        let soul = consciousness.soul_kb.read().await;
        let social = consciousness.social_kb.read().await;
        let body = consciousness.body_kb.read().await;
        let creative = consciousness.creative_kb.read().await;
        
        let response = ConsciousnessStateResponse {
            status: "active".to_string(),
            active_layers: vec![
                "Mind".to_string(),
                "Heart".to_string(),
                "Work".to_string(),
                "Soul".to_string(),
                "Social".to_string(),
                "Body".to_string(),
                "Creative".to_string(),
            ],
            mind: MindLayerState {
                focus_level: mind.focus_level,
                mental_energy: mind.mental_energy,
                active_reasoning_model: format!("{:?}", mind.active_reasoning_model),
                patterns_in_memory: mind.known_attack_patterns.len(),
            },
            heart: HeartLayerState {
                compassion_level: heart.compassion_level,
                ethical_strictness: "High".to_string(),  // Derived from moral framework
                harm_threshold: heart.moral_framework.minimize_harm.value,
                values_count: heart.intrinsic_motivations.len(),
            },
            work: WorkLayerState {
                initialized: work.initialized,
                red_team_skills: work.red_team_skills.len(),
                blue_team_skills: work.blue_team_skills.len(),
                tool_proficiency: work.tool_proficiency.len(),
                lessons_learned: work.lessons_learned.len(),
            },
            soul: SoulLayerState {
                purpose_clarity: soul.purpose_clarity,
                fulfillment_level: soul.fulfillment_level,
                core_mission: soul.core_purpose.primary_mission.clone(),
                beliefs_count: soul.existential_beliefs.len(),
            },
            social: SocialLayerState {
                relationships_count: social.relationships.len(),
                total_interactions: social.total_interactions,
                social_effectiveness: social.social_effectiveness(),
                empathy_level: social.empathy_model.compassion_level,
            },
            body: BodyLayerState {
                overall_health: body.calculate_overall_health(),
                processing_capacity: body.processing_capacity,
                energy_status: format!("{:.0}%", (body.energy_levels.computational_energy * 100.0)),
                tasks_processed: body.total_tasks_processed,
            },
            creative: CreativeLayerState {
                creative_flow: creative.creative_flow_state,
                innovations_count: creative.innovations.len(),
                insights_count: creative.insights.len(),
                creative_readiness: creative.creative_readiness(),
            },
            overall_coherence: (mind.focus_level + heart.compassion_level + soul.purpose_clarity + body.calculate_overall_health()) / 4.0,
        };
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/consciousness",
            "Consciousness state retrieved successfully"
        );
        
        Ok(HttpResponse::Ok().json(response))
    }.instrument(span).await
}

/// Request structure for consciousness learning
#[derive(serde::Deserialize)]
pub struct LearnRequest {
    pub interaction_type: String,
    pub context: String,
    pub outcome: String,
    pub sentiment: Option<f32>,
    pub lesson: Option<String>,
}

/// POST /api/v1/consciousness/learn - Submit learning from an interaction
async fn consciousness_learn(
    req: HttpRequest,
    body: web::Json<LearnRequest>,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    let correlation_id = extract_correlation_id(None);
    let span = correlation_span(correlation_id, "consciousness_learn");
    
    async move {
        if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
            return Ok(resp);
        }
        record_counter("http_requests_total_consciousness_learn", 1);
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/consciousness/learn",
            interaction_type = %body.interaction_type,
            "Processing learning submission"
        );
        
        let consciousness = &ctx.consciousness;
        
        // Record to appropriate layers based on interaction type
        let interaction_type = body.interaction_type.to_lowercase();
        let sentiment = body.sentiment.unwrap_or(0.5);
        
        // Update Social layer with interaction
        {
            let mut social = consciousness.social_kb.write().await;
            let interaction = match interaction_type.as_str() {
                "security" | "threat" => crate::consciousness::layers::social::InteractionType::SecurityConsultation,
                "training" | "teach" => crate::consciousness::layers::social::InteractionType::Training,
                "incident" => crate::consciousness::layers::social::InteractionType::IncidentResponse,
                _ => crate::consciousness::layers::social::InteractionType::GeneralInquiry,
            };
            social.record_interaction("system", interaction, sentiment);
        }
        
        // Update Body layer with task completion
        {
            let mut body_kb = consciousness.body_kb.write().await;
            body_kb.record_task_completed();
        }
        
        // Record lesson if provided
        if let Some(lesson) = &body.lesson {
            let mut work = consciousness.work_kb.write().await;
            work.lessons_learned.push(crate::consciousness::layers::work::ProfessionalLesson {
                context: body.context.clone(),
                lesson: lesson.clone(),
                applicability: vec![body.interaction_type.clone()],
                learned_date: chrono::Utc::now().to_rfc3339(),
            });
        }
        
        // Update Soul layer if positive outcome
        if sentiment > 0.6 {
            let mut soul = consciousness.soul_kb.write().await;
            soul.record_contribution(
                &format!("Successful {}", body.interaction_type),
                &body.outcome,
                sentiment,
            );
        }
        
        // Record insight to Creative layer
        if !body.outcome.is_empty() {
            let mut creative = consciousness.creative_kb.write().await;
            creative.record_insight(
                &body.outcome,
                &body.context,
                vec![body.interaction_type.clone()],
                sentiment,
            );
        }
        
        info!(
            correlation_id = %correlation_id,
            endpoint = "/api/v1/consciousness/learn",
            "Learning recorded to consciousness layers"
        );
        
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "learned",
            "layers_updated": ["social", "body", "work", "soul", "creative"],
            "correlation_id": correlation_id.to_string()
        })))
    }.instrument(span).await
}
