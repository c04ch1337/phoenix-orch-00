use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use platform::audit::{AuditLogger, AuditEventType};
use std::sync::Arc;
use std::net::IpAddr;

#[derive(Clone)]
pub struct SecurityAuditMiddleware {
    logger: Arc<AuditLogger>,
}

impl SecurityAuditMiddleware {
    pub fn new() -> Self {
        Self {
            logger: Arc::new(AuditLogger::new()),
        }
    }
}
impl<S, B> Transform<S, ServiceRequest> for SecurityAuditMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SecurityAuditMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityAuditMiddlewareService {
            service,
            logger: self.logger.clone(),
        }))
    }
}

pub struct SecurityAuditMiddlewareService<S> {
    service: S,
    logger: Arc<AuditLogger>,
}

impl<S, B> Service<ServiceRequest> for SecurityAuditMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let logger = self.logger.clone();
        let fut = self.service.call(req);
        
        Box::pin(async move {
            let start_time = std::time::Instant::now();
            let response = fut.await?;
            let duration = start_time.elapsed();

            // Extract request details
            let method = response.request().method().to_string();
            let path = response.request().path().to_string();
            let status = response.status().as_u16();
            
            // Extract user ID from JWT claims if available
            let user_id = response
                .request()
                .extensions()
                .get::<super::auth::Claims>()
                .map(|claims| claims.sub.clone());

            // Extract client IP
            let ip_address = response
                .request()
                .connection_info()
                .realip_remote_addr()
                .map(|ip| ip.to_string());

            // Create audit details
            let details = serde_json::json!({
                "method": method,
                "path": path,
                "status_code": status,
                "duration_ms": duration.as_millis(),
                "user_agent": response.request()
                    .headers()
                    .get("User-Agent")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("unknown"),
            });

            // Log the API access
            // Clone values that will be used multiple times
            let path_clone = path.clone();
            let user_id_clone = user_id.clone();
            let ip_address_clone = ip_address.clone();

            logger
                .log_api_access(
                    user_id,
                    ip_address,
                    path,
                    method,
                    status,
                    Some(details),
                )
                .await;

            // Log authentication events
            if path_clone.contains("/auth") {
                logger
                    .log_auth_attempt(
                        user_id_clone,
                        ip_address_clone,
                        status == 200,
                        Some(serde_json::json!({
                            "status_code": status,
                            "path": path_clone
                        })),
                    )
                    .await;
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use actix_web::{web, App, HttpResponse};

    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok().finish()
    }

    #[actix_web::test]
    async fn test_audit_middleware() {
        let middleware = SecurityAuditMiddleware::new();
        
        let app = test::init_service(
            App::new()
                .wrap(middleware)
                .route("/test", web::get().to(test_handler))
        ).await;

        let req = test::TestRequest::get().uri("/test").to_request();
        let resp = test::call_service(&app, req).await;
        
        assert!(resp.status().is_success());
    }
}