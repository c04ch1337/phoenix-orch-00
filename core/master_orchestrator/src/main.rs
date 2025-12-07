use actix_cors::Cors;
use actix_web::{middleware::DefaultHeaders, web, App, HttpServer, http::header};
use rusqlite::Connection;
use std::env;
use std::sync::Arc;

mod api;
mod config_service;
mod executor;
mod memory;
mod memory_service;
mod planner;
mod tool_registry_service;
mod tool_service;

use crate::api::ApiContext;
use memory_service::MemoryService;
use shared_types::AppConfig;
use tool_service::ToolService;

#[derive(serde::Deserialize, Debug)]
pub struct ChatPayload {
    pub message: String,
    pub context: Option<String>,
}

#[derive(serde::Serialize)]
pub struct ChatResponse {
    pub status: String,
    pub output: String,
}

#[derive(serde::Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub llm_provider: String,
    pub llm_model: String,
}

/// Build the Actix HTTP server for the orchestrator, wiring in HTTP + WS APIs
/// and static frontend serving. This function does not start the server; the
/// caller is responsible for awaiting the returned `Server` and coordinating
/// shutdown.
fn run_http_server(
    api_ctx: ApiContext,
    bind_addr: &str,
) -> std::io::Result<actix_web::dev::Server> {
    // Clone context so it can be moved into the factory closure.
    let ctx = api_ctx.clone();

    // Initialize middlewares
    let rate_limiter = api::rate_limit::RateLimitMiddleware::new(ctx.rate_limit_config.clone());
    let request_validator = api::validation::RequestValidationMiddleware::new();
    let security_audit = api::audit_middleware::SecurityAuditMiddleware::new();

    let server = HttpServer::new(move || {
        // 1. Configure CORS for the frontend.
        //
        // In dev we allow all origins so that both http://127.0.0.1 and
        // http://localhost work reliably, and we log the incoming Origin
        // for easier debugging.
        let cors = Cors::default()
            .allowed_origin_fn(|origin, _req_head| {
                println!("[CORS DEBUG] incoming Origin = {:?}", origin);
                true // allow all origins in dev
            })
            .allowed_methods(vec!["GET", "POST", "OPTIONS"])
            .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
            .supports_credentials()
            .max_age(3600);

        // 2. Baseline security headers for all HTTP responses.
        let csp_value = "default-src 'self'; \
script-src 'self' 'unsafe-inline'; \
connect-src 'self' http://127.0.0.1:8282 http://localhost:8282; \
img-src 'self' data: https://grainy-gradients.vercel.app; \
style-src 'self' https://fonts.googleapis.com 'unsafe-inline'; \
font-src 'self' https://fonts.gstatic.com https://fonts.googleapis.com; \
frame-ancestors 'none';";

        let security_headers = DefaultHeaders::new()
            .add(("X-Frame-Options", "DENY"))
            .add(("X-Content-Type-Options", "nosniff"))
            .add(("Referrer-Policy", "no-referrer"))
            .add(("Content-Security-Policy", csp_value));

        let api_ctx_clone = ctx.clone();

        App::new()
            .app_data(web::Data::new(api_ctx_clone.clone()))
            .configure(|cfg| {
                api::configure_http(cfg, api_ctx_clone.clone());
                api::configure_ws(cfg, api_ctx_clone.clone());
            })
            .wrap(security_headers)
            .wrap(cors)
            .service(actix_files::Files::new("/", "../../frontend").index_file("index.html"))
            .app_data(web::Data::new(api_ctx_clone.clone()))
            .configure(|cfg| {
                api::configure_http(cfg, api_ctx_clone.clone());
                api::configure_ws(cfg, api_ctx_clone.clone());
            })
            .service(actix_files::Files::new("/", "../../frontend").index_file("index.html"))
    })
    .bind(bind_addr)?
    .run();

    Ok(server)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize tracing/logging for the orchestrator.
    platform::init_tracing("master_orchestrator").expect("failed to init tracing");

    // Initialize metrics exporter on a dedicated port, if possible.
    let metrics_addr = env::var("METRICS_ADDR").unwrap_or_else(|_| "127.0.0.1:9000".to_string());
    match metrics_addr.parse() {
        Ok(addr) => {
            if let Err(e) = platform::init_metrics(addr) {
                eprintln!(
                    "[WARN] Failed to initialize metrics exporter on {}: {}",
                    metrics_addr, e
                );
            } else {
                println!("[INFO] Metrics exporter listening on {}", metrics_addr);
            }
        }
        Err(e) => {
            eprintln!(
                "[WARN] Invalid METRICS_ADDR '{}': {} (metrics exporter disabled)",
                metrics_addr, e
            );
        }
    }

    println!("Master Orchestrator Starting...");
    println!(
        "Current directory: {}",
        env::current_dir().unwrap().display()
    );

    // Load Configuration with environment overlay
    let current_exe = std::env::current_exe().unwrap();
    let current_dir = current_exe.parent().unwrap();
    let project_root = current_dir.parent().unwrap().parent().unwrap();
    let base_config_path = project_root.join("data/config.toml");
    println!("Base config path: {}", base_config_path.display());

    let app_env = env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    println!("APP_ENV={}", app_env);

    let app_config = match config_service::load_app_config_with_env(
        base_config_path.to_str().unwrap(),
        &app_env,
    ) {
        Ok(config) => {
            println!("Configuration loaded successfully (env={}).", app_env);
            println!("Default LLM Provider: {}", config.llm.default_provider);

            // Fail fast if critical LLM config is missing or invalid for the default provider
            if config.llm.default_provider == "openrouter" {
                match &config.llm.openrouter {
                    Some(p) => {
                        let key_ok = p
                            .api_key
                            .as_ref()
                            .map(|k| !k.trim().is_empty())
                            .unwrap_or(false);
                        if !key_ok {
                            eprintln!(
                                "[FATAL] OPENROUTER_API_KEY is missing or empty for default provider=openrouter"
                            );
                            return Ok(());
                        }
                        if p.base_url
                            .as_ref()
                            .map(|u| u.trim().is_empty())
                            .unwrap_or(true)
                        {
                            eprintln!(
                                "[FATAL] OpenRouter base_url is missing for default provider=openrouter"
                            );
                            return Ok(());
                        }
                    }
                    None => {
                        eprintln!(
                            "[FATAL] llm.openrouter config is missing while default_provider=openrouter"
                        );
                        return Ok(());
                    }
                }
            }

            Arc::new(config)
        }
        Err(e) => {
            eprintln!("[FATAL] Failed to load configuration: {}", e);
            return Ok(());
        }
    };

    // Initialize GAI Memory with Sled
    let sqlite_path = project_root.join("data/memory_kg.db");
    let sled_path = project_root.join("data/sled/semantic_memory");
    println!("SQLite path: {}", sqlite_path.display());
    println!("Sled path: {}", sled_path.display());

    let memory_service =
        match MemoryService::new(sqlite_path.to_str().unwrap(), sled_path.to_str().unwrap()) {
            Ok(service) => Arc::new(service),
            Err(e) => {
                eprintln!("Failed to initialize memory service: {}", e);
                return Ok(());
            }
        };

    // Initialize the tool registry database
    if let Err(e) = memory_service.initialize_tool_registry() {
        eprintln!("[FATAL] Failed to initialize tool registry database: {}", e);
        return Ok(());
    }
    println!("Tool registry database initialized successfully.");

    if let Err(e) = memory_service.init_gai_memory().await {
        eprintln!("Failed to initialize GAI memory tables: {}", e);
        return Ok(());
    }
    println!(
        "GAI Memory initialized (SQLite: {}, Sled: {})",
        sqlite_path.display(),
        sled_path.display()
    );

    // Initialize ToolService using a dedicated SQLite connection for tool registry.
    let tool_registry_conn = match Connection::open(sqlite_path.to_str().unwrap()) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("[FATAL] Failed to open tool registry database: {}", e);
            return Ok(());
        }
    };

    let tool_service = match ToolService::new(&tool_registry_conn) {
        Ok(service) => {
            println!("[INFO] ToolService initialized.");
            Arc::new(service)
        }
        Err(e) => {
            eprintln!("[FATAL] Failed to initialize ToolService: {}", e);
            return Ok(());
        }
    };
    println!(
        "[INFO] Loaded tools: {:?}",
        tool_service.tools.keys().collect::<Vec<_>>()
    );

    // Register Agents
    if let Err(e) = memory_service
        .register_agent("git_agent", "git_agent", "Handles Git operations")
        .await
    {
        eprintln!("Failed to register git_agent: {}", e);
    }
    if let Err(e) = memory_service
        .register_agent(
            "obsidian_agent",
            "obsidian_agent",
            "Handles Obsidian integration",
        )
        .await
    {
        eprintln!("Failed to register obsidian_agent: {}", e);
    }
    if let Err(e) = memory_service
        .register_agent(
            "llm_router_agent",
            "llm_router_agent",
            "Routes requests to LLM providers",
        )
        .await
    {
        eprintln!("Failed to register llm_router_agent: {}", e);
    }

    // Verify Agents
    match memory_service.get_active_agents().await {
        Ok(agents) => {
            println!("Active Agents: {:?}", agents);
        }
        Err(e) => eprintln!("Failed to get active agents: {}", e),
    }

    // --- (A) BINDING TO THE PERMANENT PORT ---
    // Bind to both IPv4 and IPv6 localhost interfaces
    const BIND_ADDRESS: &str = "127.0.0.1:8282";
    println!("🚀 Starting API server on: {}", BIND_ADDRESS);

    // Initialize JWT authentication if secret is configured
    let jwt_auth = match env::var("JWT_SECRET") {
        Ok(secret) if !secret.trim().is_empty() => {
            println!("[INFO] Initializing JWT authentication");
            Some(Arc::new(api::auth::JwtAuth::new(secret.as_bytes())))
        }
        Ok(_) => {
            println!("[WARN] JWT_SECRET is empty, authentication will be disabled");
            None
        }
        Err(_) => {
            println!("[WARN] JWT_SECRET not set, authentication will be disabled");
            None
        }
    };

    // Configure rate limiting
    let rate_limit_config = api::rate_limit::RateLimitConfig {
        requests: std::num::NonZeroU32::new(
            env::var("RATE_LIMIT_REQUESTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
        ).unwrap(),
        window_secs: env::var("RATE_LIMIT_WINDOW")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60),
    };
    println!("[INFO] Rate limiting configured: {} requests per {} seconds",
        rate_limit_config.requests, rate_limit_config.window_secs);

    // Build shared API context
    let api_ctx = ApiContext {
        memory_service: memory_service.clone(),
        app_config: app_config.clone(),
        tool_service: tool_service.clone(),
        jwt_auth,
        rate_limit_config,
    };

    // Start HTTP server with graceful shutdown on CTRL+C.
    let server = run_http_server(api_ctx, BIND_ADDRESS)?;
    let handle = server.handle();

    let shutdown_fut = async move {
        if let Err(e) = tokio::signal::ctrl_c().await {
            eprintln!("[WARN] Failed to install CTRL+C handler: {}", e);
            return;
        }
        println!("[INFO] Received CTRL+C, initiating graceful shutdown...");
        handle.stop(true).await;
    };

    tokio::select! {
        res = server => {
            if let Err(e) = res {
                eprintln!("[ERROR] HTTP server error: {}", e);
            }
        }
        _ = shutdown_fut => {
            println!("[INFO] Shutdown signal handled.");
        }
    }

    // Placeholder for future flush logic; currently a no-op.
    memory_service.shutdown().await;

    Ok(())
}
