use super::{auth::verify_auth, ApiContext};
use actix::{Actor, ActorContext, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde_json::error::Error as SerdeError;
use shared_types::{
    ClientToOrchestratorWsMessageV1, OrchestratorError, OrchestratorErrorCode,
    OrchestratorToClientWsMessageV1,
};
use super::auth::Claims;

pub fn configure(cfg: &mut web::ServiceConfig, ctx: ApiContext) {
    let ctx_data = web::Data::new(ctx);
    cfg.app_data(ctx_data.clone())
        .route("/ws", web::get().to(ws_entry));
}

struct WsSession {
    api_ctx: ApiContext,
    claims: Option<Claims>,
}

impl WsSession {
    pub fn new(api_ctx: ApiContext, claims: Option<Claims>) -> Self {
        Self { api_ctx, claims }
    }

    fn handle_incoming_message(
        &self,
        text: &str,
    ) -> Result<Option<OrchestratorToClientWsMessageV1>, SerdeError> {
        let msg: ClientToOrchestratorWsMessageV1 = serde_json::from_str(text)?;
        match msg {
            ClientToOrchestratorWsMessageV1::Chat(_req) => {
                // For now, we do not execute chat over WS; HTTP remains the primary path.
                // A future implementation could call into planner::plan_and_execute_v1 here
                // and stream intermediate updates.
                Ok(None)
            }
            ClientToOrchestratorWsMessageV1::SubscribePlan { .. } => {
                // Stub: in a full implementation, we would register this session for plan updates.
                Ok(None)
            }
            ClientToOrchestratorWsMessageV1::UnsubscribePlan { .. } => {
                // Stub: in a full implementation, we would unregister this session.
                Ok(None)
            }
        }
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Text(text)) => {
                let text_str = text.trim();
                if text_str.is_empty() {
                    return;
                }

                match self.handle_incoming_message(text_str) {
                    Ok(Some(outgoing)) => {
                        if let Ok(json) = serde_json::to_string(&outgoing) {
                            ctx.text(json);
                        }
                    }
                    Ok(None) => {
                        // No response for this message type in the minimal stub.
                    }
                    Err(e) => {
                        // Send a structured error back to the client.
                        let err = OrchestratorError {
                            code: OrchestratorErrorCode::ValidationFailed,
                            message: format!("Invalid WS message: {}", e),
                            details: None,
                        };
                        let outbound = OrchestratorToClientWsMessageV1::Error {
                            correlation_id: uuid::Uuid::new_v4(),
                            error: err,
                        };
                        if let Ok(json) = serde_json::to_string(&outbound) {
                            ctx.text(json);
                        }
                    }
                }
            }
            Ok(ws::Message::Ping(msg)) => {
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Binary(_bin)) => {
                // Binary not supported; ignore.
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Continuation(_)) => {}
            Ok(ws::Message::Nop) => {
                // No-op keepalive message; ignore.
            }
            Err(_e) => {
                ctx.stop();
            }
        }
    }
}

async fn ws_entry(
    req: HttpRequest,
    stream: web::Payload,
    ctx: web::Data<ApiContext>,
) -> Result<HttpResponse, Error> {
    // Verify JWT token if authentication is enabled
    let claims = if let Some(jwt_auth) = &ctx.get_ref().jwt_auth {
        match verify_auth(&req, jwt_auth).await {
            Ok(claims) => Some(claims),
            Err(e) => return Ok(HttpResponse::Unauthorized().json(e.to_string())),
        }
    } else {
        None
    };

    let session = WsSession::new(ctx.get_ref().clone(), claims);
    ws::start(session, &req, stream)
}
