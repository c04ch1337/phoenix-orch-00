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
