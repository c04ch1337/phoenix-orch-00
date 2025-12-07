use actix_web::{test, web, App};
use master_orchestrator::{
    api, config_service, memory_service::MemoryService, tool_service::ToolService,
};
use shared_types::{ChatRequestV1, ChatResponseV1, API_VERSION_CURRENT};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

#[actix_web::test]
async fn smoke_chat_v1_returns_well_formed_response() {
    // Load base + dev overlay config similar to main.rs, but using CARGO_MANIFEST_DIR.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir.parent().unwrap().parent().unwrap();
    let config_path = project_root.join("data/config.toml");
    let config_path_str = config_path.to_str().expect("valid config path");

    // It is safe for this test to rely on placeholder keys; no real network calls occur.
    let app_config = config_service::load_app_config_with_env(config_path_str, "dev")
        .expect("config should load for dev");
    let app_config = Arc::new(app_config);

    // Use a temporary directory for SQLite and Sled storage to keep the test isolated.
    let temp_dir = TempDir::new().expect("temp dir");
    let sqlite_path = temp_dir.path().join("memory_kg.db");
    let sled_path = temp_dir.path().join("sled");

    let memory_service = Arc::new(
        MemoryService::new(
            sqlite_path.to_str().expect("sqlite path utf8"),
            sled_path.to_str().expect("sled path utf8"),
        )
        .expect("memory service should initialize"),
    );

    // Initialize the schema used by the planner for agent registry and health state.
    memory_service
        .init_gai_memory()
        .await
        .expect("init_gai_memory should succeed");

    // Minimal ToolService with no tools; the v1 planner path does not require tools for this test.
    let tool_service = Arc::new(ToolService {
        tools: HashMap::new(),
    });

    let api_ctx = api::ApiContext {
        memory_service: memory_service.clone(),
        app_config: app_config.clone(),
        tool_service: tool_service.clone(),
        auth_token: None,
    };

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(api_ctx.clone()))
            .configure(|cfg| {
                api::configure_http(cfg, api_ctx.clone());
            }),
    )
    .await;

    let request_body = ChatRequestV1 {
        api_version: API_VERSION_CURRENT,
        correlation_id: None,
        message: "hello from integration test".to_string(),
        context: None,
    };

    let req = test::TestRequest::post()
        .uri("/api/v1/chat")
        .set_json(&request_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(
        resp.status().is_success(),
        "expected HTTP 200 from /api/v1/chat"
    );

    let body_bytes = test::read_body(resp).await;
    let chat_resp: ChatResponseV1 =
        serde_json::from_slice(&body_bytes).expect("response should deserialize as ChatResponseV1");

    assert_eq!(chat_resp.api_version, API_VERSION_CURRENT);
    assert!(
        !chat_resp.status.is_empty(),
        "status field should be non-empty"
    );

    // The orchestrator must always assign a non-nil correlation ID.
    assert_ne!(chat_resp.correlation_id, Uuid::nil());
}
