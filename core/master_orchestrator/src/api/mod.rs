use actix_web::web;
use std::sync::Arc;

use crate::memory_service::MemoryService;
use crate::tool_service::ToolService;
use shared_types::AppConfig;

pub mod audit_middleware;
pub mod auth;
pub mod http;
pub mod rate_limit;
pub mod validation;
pub mod ws;

use auth::JwtAuth;
use rate_limit::RateLimitConfig;

#[derive(Clone)]
pub struct ApiContext {
    pub memory_service: Arc<MemoryService>,
    pub app_config: Arc<AppConfig>,
    pub tool_service: Arc<ToolService>,
    /// JWT authentication handler
    pub jwt_auth: Option<Arc<JwtAuth>>,
    /// Rate limiting configuration
    pub rate_limit_config: RateLimitConfig,
    /// Current application environment (dev, staging, prod)
    pub app_env: String,
}

pub fn configure_http(cfg: &mut web::ServiceConfig, ctx: ApiContext) {
    http::configure(cfg, ctx);
}

pub fn configure_ws(cfg: &mut web::ServiceConfig, ctx: ApiContext) {
    ws::configure(cfg, ctx);
}
