use std::sync::Arc;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::ApiContext;
use crate::memory_service::MemoryService;
use crate::planner;
use crate::tool_service::ToolService;
use platform::record_counter;
use shared_types::{AppConfig, ChatRequestV1, ChatResponseV1, CorrelationId, API_VERSION_CURRENT};

/// Legacy chat payload and response types used by `/api/chat`.
#[derive(Deserialize, Debug)]
pub struct ChatPayload {
    pub message: String,
    pub context: Option<String>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub status: String,
    pub output: String,
}

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
        .route("/api/chat", web::post().to(chat_legacy))
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

// Legacy /api/chat endpoint - preserves existing behavior and payload shape.
async fn chat_legacy(
    req: HttpRequest,
    payload: web::Json<ChatPayload>,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
        return Ok(resp);
    }
    record_counter("http_requests_total_chat_legacy", 1);

    println!("[INFO] Received /api/chat request: {:?}", payload);

    let memory_service: Arc<MemoryService> = ctx.memory_service.clone();
    let app_config: Arc<AppConfig> = ctx.app_config.clone();
    let tool_service: Arc<ToolService> = ctx.tool_service.clone();

    match planner::plan_and_execute(
        payload.message.clone(),
        memory_service,
        app_config,
        tool_service,
    )
    .await
    {
        Ok(response) => {
            let chat_response = ChatResponse {
                status: "success".to_string(),
                output: serde_json::to_string(&response.result).unwrap_or_default(),
            };
            Ok(HttpResponse::Ok().json(chat_response))
        }
        Err(e) => {
            eprintln!("[ERROR] plan_and_execute failed: {}", e);
            let chat_response = ChatResponse {
                status: "error".to_string(),
                output: e,
            };
            // Keep HTTP 200 for front-end compatibility, but payload clearly indicates error
            Ok(HttpResponse::Ok().json(chat_response))
        }
    }
}

// New versioned /api/v1/chat endpoint using shared ChatRequestV1/ChatResponseV1 contracts.
async fn chat_v1(
    req: HttpRequest,
    body: web::Json<ChatRequestV1>,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
        return Ok(resp);
    }
    record_counter("http_requests_total_chat_v1", 1);

    handle_chat(body.into_inner(), ctx).await
}

async fn handle_chat(
    mut req: ChatRequestV1,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    let correlation_id: CorrelationId = req.correlation_id.unwrap_or_else(uuid::Uuid::new_v4);

    req.correlation_id = Some(correlation_id);

    let result = planner::plan_and_execute_v1(
        correlation_id,
        req.message.clone(),
        req.context.clone(),
        ctx.memory_service.clone(),
        ctx.app_config.clone(),
        ctx.tool_service.clone(),
    )
    .await;

    match result {
        Ok(out) => {
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
}

async fn list_agents(req: HttpRequest, ctx: web::Data<ApiContext>) -> Result<HttpResponse, Error> {
    if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
        return Ok(resp);
    }
    record_counter("http_requests_total_agents_v1", 1);

    let summaries = ctx
        .memory_service
        .list_agent_health()
        .await
        .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(summaries))
}

async fn health(req: HttpRequest, ctx: web::Data<ApiContext>) -> Result<HttpResponse, Error> {
    if let Err(resp) = require_auth(&req, ctx.get_ref()).await {
        return Ok(resp);
    }
    record_counter("http_requests_total_health", 1);

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

    let health = HealthResponse {
        status: "ok",
        llm_provider: provider,
        llm_model: model,
    };

    Ok(HttpResponse::Ok().json(health))
}
