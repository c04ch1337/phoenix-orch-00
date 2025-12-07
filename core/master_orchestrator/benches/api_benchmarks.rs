use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use master_orchestrator::{
    api, config_service, memory_service::MemoryService, tool_service::ToolService,
};
use shared_types::{ChatRequestV1, API_VERSION_CURRENT};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use actix_web::{test, web, App};

async fn setup_test_app() -> (actix_web::test::TestServer, api::ApiContext) {
    // Load test config
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir.parent().unwrap().parent().unwrap();
    let config_path = project_root.join("data/config.toml");
    let config_path_str = config_path.to_str().expect("valid config path");
    
    let app_config = config_service::load_app_config_with_env(config_path_str, "dev")
        .expect("config should load for dev");
    let app_config = Arc::new(app_config);

    // Use temp directory for test storage
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

    memory_service
        .init_gai_memory()
        .await
        .expect("init_gai_memory should succeed");

    let tool_service = Arc::new(ToolService {
        tools: HashMap::new(),
    });

    let api_ctx = api::ApiContext {
        memory_service: memory_service.clone(),
        app_config: app_config.clone(),
        tool_service: tool_service.clone(),
        auth_token: None,
    };

    let server = test::init_service(
        App::new()
            .app_data(web::Data::new(api_ctx.clone()))
            .configure(|cfg| {
                api::configure_http(cfg, api_ctx.clone());
            }),
    )
    .await;

    (server, api_ctx)
}

fn chat_api_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("chat_api");
    group.sample_size(50); // Increase sample size for more stable results
    group.measurement_time(std::time::Duration::from_secs(30));

    // Benchmark different message sizes
    let message_sizes = vec![10, 100, 1000];
    
    for size in message_sizes {
        group.bench_with_input(
            BenchmarkId::new("message_size", size),
            &size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (app, _) = setup_test_app().await;
                    
                    let message = "x".repeat(size);
                    let request = ChatRequestV1 {
                        api_version: API_VERSION_CURRENT,
                        correlation_id: None,
                        message,
                        context: None,
                    };

                    let req = test::TestRequest::post()
                        .uri("/api/v1/chat")
                        .set_json(&request)
                        .to_request();

                    test::call_service(&app, req).await
                });
            },
        );
    }

    // Benchmark concurrent requests
    let concurrent_counts = vec![1, 5, 10];
    
    for count in concurrent_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_requests", count),
            &count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let (app, _) = setup_test_app().await;
                    
                    let futures = (0..count).map(|_| {
                        let request = ChatRequestV1 {
                            api_version: API_VERSION_CURRENT,
                            correlation_id: None,
                            message: "benchmark test".to_string(),
                            context: None,
                        };

                        let req = test::TestRequest::post()
                            .uri("/api/v1/chat")
                            .set_json(&request)
                            .to_request();

                        test::call_service(&app, req)
                    });

                    futures_util::future::join_all(futures).await
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, chat_api_benchmarks);
criterion_main!(benches);