use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpResponse, HttpMessage,
    body::{BoxBody, MessageBody, EitherBody},
};
use actix_web::http::Method;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use std::sync::Arc;

/// Validation schemas for different API endpoints
pub struct ValidationSchemas {
    chat_schema: Arc<JSONSchema>,
    ws_schema: Arc<JSONSchema>,
}

impl ValidationSchemas {
    pub fn new() -> Self {
        // Chat request schema
        let schema_value = serde_json::json!({
            "type": "object",
            "required": ["message"],
            "properties": {
                "message": {
                    "type": "string",
                    "minLength": 1
                },
                "context": {
                    "type": ["string", "null"]
                },
                "correlation_id": {
                    "type": ["string", "null"],
                    "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"
                }
            },
            "additionalProperties": false
        });
        let chat_schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .expect("Invalid chat schema");

        // WebSocket message schema
        let ws_schema_value = serde_json::json!({
            "type": "object",
            "required": ["type"],
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["chat", "subscribe_plan", "unsubscribe_plan"]
                },
                "payload": {
                    "type": "object"
                }
            },
            "additionalProperties": false
        });
        let ws_schema = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&ws_schema_value)
            .expect("Invalid WebSocket schema");

        Self {
            chat_schema: Arc::new(chat_schema),
            ws_schema: Arc::new(ws_schema),
        }
    }

    pub fn validate_chat(&self, value: &Value) -> Result<(), String> {
        self.chat_schema
            .validate(value)
            .map_err(|errors| {
                errors
                    .map(|e| format!("{} at {}", e.to_string(), e.instance_path))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
    }

    pub fn validate_ws(&self, value: &Value) -> Result<(), String> {
        self.ws_schema
            .validate(value)
            .map_err(|errors| {
                errors
                    .map(|e| format!("{} at {}", e.to_string(), e.instance_path))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
    }
}

#[derive(Clone)]
pub struct RequestValidationMiddleware {
    schemas: Arc<ValidationSchemas>,
}

impl RequestValidationMiddleware {
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(ValidationSchemas::new()),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RequestValidationMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + Clone + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RequestValidationMiddlewareService<S, B>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestValidationMiddlewareService::new(
            service,
            self.schemas.clone(),
        )))
    }
}

#[derive(Clone)]
pub struct RequestValidationMiddlewareService<S, B> {
    service: S,
    schemas: Arc<ValidationSchemas>,
    _phantom: std::marker::PhantomData<B>,
}

impl<S, B> RequestValidationMiddlewareService<S, B> {
    fn new(service: S, schemas: Arc<ValidationSchemas>) -> Self {
        Self {
            service,
            schemas,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<S, B> Service<ServiceRequest> for RequestValidationMiddlewareService<S, B>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + Clone + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let schemas = self.schemas.clone();
        let method = req.method().clone();
        let path = req.path().to_owned();

        if method == Method::POST && path.starts_with("/api") {
            let service = self.service.clone();
            Box::pin(async move {
                if let Ok(body) = req.extract::<web::Json<Value>>().await {
                    if path.ends_with("/chat") {
                        if let Err(err) = schemas.validate_chat(&body.into_inner()) {
                            let res = HttpResponse::BadRequest()
                                .body(err.to_string())
                                .map_into_right_body();
                            return Ok(ServiceResponse::new(req.into_parts().0, res));
                        }
                    }
                } else {
                    let res = HttpResponse::BadRequest()
                        .finish()
                        .map_into_right_body();
                    return Ok(ServiceResponse::new(req.into_parts().0, res));
                }
                Ok(service.call(req).await?.map_into_left_body())
            })
        } else {
            let fut = self.service.call(req);
            Box::pin(async move { Ok(fut.await?.map_into_left_body()) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_chat_validation() {
        let schemas = ValidationSchemas::new();

        // Valid chat request
        let valid = json!({
            "message": "Hello",
            "context": null,
            "correlation_id": "550e8400-e29b-41d4-a716-446655440000"
        });
        assert!(schemas.validate_chat(&valid).is_ok());

        // Invalid - empty message
        let invalid = json!({
            "message": "",
            "context": null
        });
        assert!(schemas.validate_chat(&invalid).is_err());

        // Invalid - missing required field
        let invalid = json!({
            "context": "test"
        });
        assert!(schemas.validate_chat(&invalid).is_err());

        // Invalid - wrong correlation_id format
        let invalid = json!({
            "message": "Hello",
            "correlation_id": "invalid-uuid"
        });
        assert!(schemas.validate_chat(&invalid).is_err());
    }

    #[test]
    fn test_ws_validation() {
        let schemas = ValidationSchemas::new();

        // Valid WS message
        let valid = json!({
            "type": "chat",
            "payload": {
                "message": "Hello"
            }
        });
        assert!(schemas.validate_ws(&valid).is_ok());

        // Invalid - unknown type
        let invalid = json!({
            "type": "unknown",
            "payload": {}
        });
        assert!(schemas.validate_ws(&invalid).is_err());

        // Invalid - missing type
        let invalid = json!({
            "payload": {}
        });
        assert!(schemas.validate_ws(&invalid).is_err());
    }
}