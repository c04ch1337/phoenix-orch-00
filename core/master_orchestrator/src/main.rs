use actix_web::{web, App, HttpServer, HttpResponse, Error};
use actix_cors::Cors;
use std::sync::Arc;

mod executor;
mod planner;
mod memory_service;
mod config_service;

use memory_service::MemoryService;

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

use shared_types::AppConfig;

// --- ACTIX-WEB HANDLER FUNCTION ---
async fn chat_endpoint(
    payload: web::Json<ChatPayload>,
    memory_service: web::Data<Arc<MemoryService>>,
    app_config: web::Data<Arc<AppConfig>>,
) -> Result<HttpResponse, Error> {
    println!("Received chat request: {:?}", payload);
    
    // Use the existing planner logic
    match planner::plan_and_execute(payload.message.clone(), memory_service.get_ref().clone(), app_config.get_ref().clone()).await {
        Ok(response) => {
            let chat_response = ChatResponse {
                status: "success".to_string(),
                output: serde_json::to_string(&response.result).unwrap_or_default(),
            };
            Ok(HttpResponse::Ok().json(chat_response))
        }
        Err(e) => {
            let chat_response = ChatResponse {
                status: "error".to_string(),
                output: e,
            };
            Ok(HttpResponse::Ok().json(chat_response)) // Return 200 even on logic error to show in UI
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env file
    dotenvy::dotenv().ok();
    
    println!("Master Orchestrator Starting...");

    // Load Configuration
    let config_path = "./data/config.toml";
    let app_config = match config_service::load_config(config_path) {
        Ok(config) => {
            println!("Configuration loaded successfully.");
            println!("Default LLM Provider: {}", config.llm.default_provider);
            Arc::new(config)
        },
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return Ok(());
        }
    };

    // Initialize GAI Memory
    let db_path = "./data/memory_kg.db";
    let memory_service = match MemoryService::new(db_path) {
        Ok(service) => Arc::new(service),
        Err(e) => {
            eprintln!("Failed to initialize memory service: {}", e);
            return Ok(());
        }
    };

    if let Err(e) = memory_service.init_gai_memory().await {
        eprintln!("Failed to initialize GAI memory tables: {}", e);
        return Ok(());
    }
    println!("GAI Memory initialized at {}", db_path);

    // Register Agents
    if let Err(e) = memory_service.register_agent("git_agent", "git_agent", "Handles Git operations").await {
        eprintln!("Failed to register git_agent: {}", e);
    }
    if let Err(e) = memory_service.register_agent("obsidian_agent", "obsidian_agent", "Handles Obsidian integration").await {
        eprintln!("Failed to register obsidian_agent: {}", e);
    }
    if let Err(e) = memory_service.register_agent("llm_router_agent", "llm_router_agent", "Routes requests to LLM providers").await {
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
    const BIND_ADDRESS: &str = "127.0.0.1:8181";
    println!("🚀 Starting API server on: {}", BIND_ADDRESS);

    // Create Actix Data for shared state
    let memory_data = web::Data::new(memory_service.clone());
    let config_data = web::Data::new(app_config.clone());

    HttpServer::new(move || {
        // 1. Configure CORS for the frontend
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(memory_data.clone())
            .app_data(config_data.clone())
            .route("/api/chat", web::post().to(chat_endpoint))
            .service(actix_files::Files::new("/", "./frontend").index_file("index.html"))
    })
    .bind(BIND_ADDRESS)?
    .run()
    .await
}
