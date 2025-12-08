use actix_cors::Cors;
use actix_web::{middleware::DefaultHeaders, web, App, HttpServer, http::header};
use rusqlite::Connection;
use std::env;
use std::sync::Arc;

mod api;
mod cache_service;
mod config_service;
mod executor;
mod memory;
mod memory_service;
mod planner;
mod redis_service;
mod tool_registry_service;
mod tool_service;

use crate::api::ApiContext;
use memory_service::MemoryService;
use shared_types::Tool;
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
    frontend_path: String,
) -> std::io::Result<actix_web::dev::Server> {
    // Clone context so it can be moved into the factory closure.
    let ctx = api_ctx.clone();
    let frontend_dir = frontend_path.clone();

    let server = HttpServer::new(move || {
        let frontend_path_clone = frontend_dir.clone();
        // 1. Configure CORS for the frontend.
        //
        // In dev we allow all origins so that both http://127.0.0.1 and
        // http://localhost work reliably, and we log the incoming Origin
        // for easier debugging.
        // Set up CORS configuration based on environment
        let cors = if ctx.app_env == "prod" {
            // In production, only allow specific origins
            Cors::default()
                .allowed_origin("https://phoenix-orch.example.com") // Update with your actual domain
                .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
                .supports_credentials()
                .max_age(3600)
        } else {
            // In development, allow localhost and 127.0.0.1 but still restrict
            Cors::default()
                .allowed_origin("http://localhost:8282")
                .allowed_origin("http://127.0.0.1:8282")
                .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                .allowed_headers(vec![header::AUTHORIZATION, header::CONTENT_TYPE])
                .supports_credentials()
                .max_age(3600)
        };

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
            .service(actix_files::Files::new("/", &frontend_path_clone).index_file("index.html"))
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
    
    // Get current directory with proper error handling
    let current_dir = match env::current_dir() {
        Ok(dir) => {
            println!("Current directory: {}", dir.display());
            dir
        },
        Err(e) => {
            eprintln!("[FATAL] Failed to determine current directory: {}", e);
            return Ok(());
        }
    };

    // Always use an absolute path to the root of the project
    // See if we're in the correct directory structure by looking for data/config.toml
    let mut project_root = current_dir.clone();
    
    // Try direct path first
    if project_root.join("data/config.toml").exists() {
        // We're already at the root
    } else if project_root.join("../data/config.toml").exists() {
        // We're one level down
        project_root = project_root.join("..").canonicalize().unwrap_or(project_root);
    } else if project_root.join("../../data/config.toml").exists() {
        // We're two levels down (e.g., in core/master_orchestrator)
        project_root = project_root.join("../..").canonicalize().unwrap_or(project_root);
    } else {
        // Hardcode the path as a fallback
        project_root = std::path::PathBuf::from("c:/Users/JAMEYMILNER/AppData/Local/phoenix-orch-00");
        println!("Could not find data/config.toml in parent directories, using hardcoded path: {}",
                 project_root.display());
    }
    
    let config_path = project_root.join("data/config.toml");
    println!("Base config path: {}", config_path.display());
    let base_config_path = config_path;

    let app_env = env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    println!("APP_ENV={}", app_env);

    // Convert path to string with proper error handling
    let config_path_str = match base_config_path.to_str() {
        Some(s) => s,
        None => {
            eprintln!("[FATAL] Config path contains invalid Unicode");
            return Ok(());
        }
    };

    let app_config = match config_service::load_app_config_with_env(
        config_path_str,
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

    // Initialize Redis cache if configured
    if let Some(redis_config) = &app_config.redis {
        if let Err(e) = redis_service::initialize_redis(Some(redis_config)) {
            eprintln!("[WARN] Failed to initialize Redis cache: {}", e);
            println!("[INFO] Continuing without Redis caching");
        } else {
            println!("[INFO] Redis cache initialized successfully: {}", redis_config.url);
        }
    } else {
        println!("[INFO] Redis configuration not found, caching disabled");
        if let Err(e) = redis_service::initialize_redis(None) {
            eprintln!("[WARN] Error marking Redis as disabled: {}", e);
        }
    }

    // Initialize GAI Memory with Sled
    // Get the base directory from config path if possible
    let base_dir = base_config_path.parent().unwrap_or(&current_dir);
    // Don't prepend "data/" since base_dir already includes it
    let sqlite_path = base_dir.join("memory_kg.db");
    let sled_path = base_dir.join("sled/semantic_memory");
    println!("SQLite path: {}", sqlite_path.display());
    println!("Sled path: {}", sled_path.display());

    // Convert paths to strings with proper error handling
    let sqlite_path_str = match sqlite_path.to_str() {
        Some(s) => s,
        None => {
            eprintln!("[FATAL] SQLite path contains invalid Unicode");
            return Ok(());
        }
    };
    
    let sled_path_str = match sled_path.to_str() {
        Some(s) => s,
        None => {
            eprintln!("[FATAL] Sled path contains invalid Unicode");
            return Ok(());
        }
    };

    let memory_service =
        match MemoryService::new(sqlite_path_str, sled_path_str) {
            Ok(service) => Arc::new(service),
            Err(e) => {
                eprintln!("[FATAL] Failed to initialize memory service: {}", e);
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
    let tool_registry_conn = match Connection::open(sqlite_path_str) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("[FATAL] Failed to open tool registry database: {}", e);
            return Ok(());
        }
    };

    let mut tool_service = match ToolService::new(&tool_registry_conn) {
        Ok(service) => {
            println!("[INFO] ToolService initialized.");
            Arc::new(service)
        }
        Err(e) => {
            eprintln!("[FATAL] Failed to initialize ToolService: {}", e);
            return Ok(());
        }
    };
    // Register tools if none are found
    if tool_service.tools.is_empty() {
        println!("[INFO] No tools found in registry, registering default tools...");
        
        // Get absolute paths to the executables in the target/debug directory
        let project_root = env::current_dir().unwrap();
        
        // Go up to project root from master_orchestrator
        let project_root = match project_root.parent() {
            Some(parent) => match parent.parent() {
                Some(root) => root.to_path_buf(),
                None => project_root.clone()
            },
            None => project_root.clone()
        };
        
        println!("[INFO] Project root for tool paths: {}", project_root.display());
        
        // Use paths relative to the project root to avoid Windows path length limits
        let llm_router_path = "target/debug/llm_router_agent.exe";
        let git_agent_path = "target/debug/git_agent.exe";
        let obsidian_agent_path = "target/debug/obsidian_agent.exe";
        
        println!("[INFO] Using relative paths to target/debug directory");
        println!("[INFO] LLM Router path: {}", llm_router_path);
        println!("[INFO] Git Agent path: {}", git_agent_path);
        println!("[INFO] Obsidian Agent path: {}", obsidian_agent_path);
        
        // Define basic tools for each agent
        let tools = vec![
            Tool {
                name: "llm_router_agent".to_string(),
                version: "0.1.0".to_string(),
                description: "Routes requests to LLM providers".to_string(),
                executable_path: llm_router_path.to_string(),
                actions_schema: serde_json::json!({}), // Simple empty schema
                tags: "llm,ai".to_string(),
                category: "ai".to_string(),
                enabled: true,
            },
            Tool {
                name: "git_agent".to_string(),
                version: "0.1.0".to_string(),
                description: "Handles Git operations".to_string(),
                executable_path: git_agent_path.to_string(),
                actions_schema: serde_json::json!({}), // Simple empty schema
                tags: "git,vcs".to_string(),
                category: "vcs".to_string(),
                enabled: true,
            },
            Tool {
                name: "obsidian_agent".to_string(),
                version: "0.1.0".to_string(),
                description: "Handles Obsidian integration".to_string(),
                executable_path: obsidian_agent_path.to_string(),
                actions_schema: serde_json::json!({}), // Simple empty schema
                tags: "obsidian,notes".to_string(),
                category: "notes".to_string(),
                enabled: true,
            }
        ];
        
        // Register each tool
        for tool in tools {
            if let Err(e) = tool_registry_service::register_tool(&tool_registry_conn, &tool) {
                eprintln!("[WARN] Failed to register tool {}: {}", tool.name, e);
            } else {
                println!("[INFO] Registered tool: {}", tool.name);
            }
        }
        
        // Reload the tool service to pick up newly registered tools
        let new_tool_service = match ToolService::new(&tool_registry_conn) {
            Ok(service) => Arc::new(service),
            Err(e) => {
                eprintln!("[ERROR] Failed to reload ToolService after registering tools: {}", e);
                return Ok(());
            }
        };
        
        // Replace the old tool service with the new one
        tool_service = new_tool_service;
    }

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

    // Configure rate limiting with safe defaults
    let rate_limit_requests = match env::var("RATE_LIMIT_REQUESTS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
    {
        Some(r) if r > 0 => r,
        _ => 100, // Default to 100 requests if not specified or invalid
    };
    
    // Ensure we have a non-zero value (required by NonZeroU32)
    let requests = match std::num::NonZeroU32::new(rate_limit_requests) {
        Some(val) => val,
        None => {
            // This should never happen due to the check above, but just in case
            eprintln!("[WARN] Invalid rate limit requests value, using default of 100");
            std::num::NonZeroU32::new(100).expect("100 is a valid non-zero value")
        }
    };
    
    let window_secs = env::var("RATE_LIMIT_WINDOW")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60); // Default to 60 seconds
    
    let rate_limit_config = api::rate_limit::RateLimitConfig {
        requests,
        window_secs,
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
        app_env: app_env.clone(),
    };

    // Build the frontend path from project root
    let frontend_path = project_root.join("frontend").to_string_lossy().to_string();
    println!("[INFO] Frontend path: {}", frontend_path);

    // Start HTTP server with graceful shutdown on CTRL+C.
    let server = run_http_server(api_ctx, BIND_ADDRESS, frontend_path)?;
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
