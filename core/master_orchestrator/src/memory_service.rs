use rusqlite::{params, Connection};
use shared_types::{ActionRequest, ActionResponse};
use std::sync::{Arc, Mutex};
use tokio::task;
use uuid::Uuid;

use crate::memory::semantic::{SemanticMemory, generate_simple_embedding};

#[derive(Debug)]
#[allow(dead_code)]
pub struct AgentConfig {
    pub id: i64,
    pub tool_name: String,
    pub binary_path: String,
    pub is_active: bool,
    pub description: String,
}

#[derive(Clone)]
pub struct MemoryService {
    conn: Arc<Mutex<Connection>>,
    semantic: Arc<SemanticMemory>,
}

impl MemoryService {
    pub fn new(db_path: &str, rocksdb_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        
        // Initialize RocksDB for semantic memory
        let semantic = SemanticMemory::init(rocksdb_path)
            .map_err(|e| format!("RocksDB init failed: {}", e))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            semantic: Arc::new(semantic),
        })
    }

    pub async fn init_gai_memory(&self) -> Result<(), String> {
        let conn = self.conn.clone();
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            
            // Create agent_registry table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS agent_registry (
                    id INTEGER PRIMARY KEY,
                    tool_name TEXT NOT NULL UNIQUE,
                    binary_path TEXT NOT NULL,
                    is_active INTEGER NOT NULL,
                    description TEXT
                )",
                [],
            ).map_err(|e| e.to_string())?;

            // Create action_trace_log table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS action_trace_log (
                    trace_id TEXT PRIMARY KEY,
                    request_json TEXT NOT NULL,
                    response_json TEXT NOT NULL,
                    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
                )",
                [],
            ).map_err(|e| e.to_string())?;

            // Layer 1: Knowledge Graph (Structured Memory)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS knowledge_graph (
                    id INTEGER PRIMARY KEY,
                    subject TEXT NOT NULL,
                    predicate TEXT NOT NULL,
                    object TEXT NOT NULL,
                    UNIQUE(subject, predicate, object)
                )",
                [],
            ).map_err(|e| e.to_string())?;

            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())??;

        // Layer 2: Semantic Memory is now handled by RocksDB (no flat file dirs needed)
        println!("Semantic Memory (RocksDB) initialized");

        Ok(())
    }

    pub async fn log_action_trace(
        &self,
        request: &ActionRequest,
        response: &ActionResponse,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let request_json = serde_json::to_string(request).unwrap_or_default();
        let response_json = serde_json::to_string(response).unwrap_or_default();
        let trace_id = request.request_id.to_string();

        // Also store as semantic memory for retrieval!
        let semantic_text = format!("Action: {} Tool: {} Prompt: {}", request.action, request.tool, request.payload.0);
        self.store_semantic_memory(&semantic_text).await?;

        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO action_trace_log (trace_id, request_json, response_json) VALUES (?1, ?2, ?3)",
                params![trace_id, request_json, response_json],
            ).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn register_agent(
        &self,
        tool_name: &str,
        binary_path: &str,
        description: &str,
    ) -> Result<(), String> {
        let conn = self.conn.clone();
        let tool_name = tool_name.to_string();
        let binary_path = binary_path.to_string();
        let description = description.to_string();

        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO agent_registry (tool_name, binary_path, is_active, description)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(tool_name) DO UPDATE SET
                    binary_path = excluded.binary_path,
                    is_active = 1,
                    description = excluded.description",
                params![tool_name, binary_path, description],
            ).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn get_active_agents(&self) -> Result<Vec<AgentConfig>, String> {
        let conn = self.conn.clone();
        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, tool_name, binary_path, is_active, description FROM agent_registry WHERE is_active = 1",
            ).map_err(|e| e.to_string())?;
            
            let agent_iter = stmt.query_map([], |row| {
                Ok(AgentConfig {
                    id: row.get(0)?,
                    tool_name: row.get(1)?,
                    binary_path: row.get(2)?,
                    is_active: row.get(3)?,
                    description: row.get(4)?,
                })
            }).map_err(|e| e.to_string())?;

            let mut agents = Vec::new();
            for agent in agent_iter {
                agents.push(agent.map_err(|e| e.to_string())?);
            }
            Ok::<Vec<AgentConfig>, String>(agents)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    // --- Layer 1: Structured Memory (KG) ---

    pub async fn add_knowledge_triple(&self, subject: &str, predicate: &str, object: &str) -> Result<(), String> {
        let conn = self.conn.clone();
        let s = subject.to_string();
        let p = predicate.to_string();
        let o = object.to_string();

        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO knowledge_graph (subject, predicate, object) VALUES (?1, ?2, ?3)",
                params![s, p, o],
            ).map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn retrieve_structured_context(&self, query: &str) -> Result<Vec<String>, String> {
        let conn = self.conn.clone();
        let q = format!("%{}%", query); // Simple LIKE query for now

        task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT subject, predicate, object FROM knowledge_graph 
                 WHERE subject LIKE ?1 OR object LIKE ?1 LIMIT 10"
            ).map_err(|e| e.to_string())?;

            let rows = stmt.query_map(params![q], |row| {
                let s: String = row.get(0)?;
                let p: String = row.get(1)?;
                let o: String = row.get(2)?;
                Ok(format!("{} {} {}", s, p, o))
            }).map_err(|e| e.to_string())?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| e.to_string())?);
            }
            Ok::<Vec<String>, String>(results)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    // --- Layer 2: Semantic Memory (RocksDB) ---

    pub async fn store_semantic_memory(&self, text: &str) -> Result<(), String> {
        let text_content = text.to_string();
        let semantic = self.semantic.clone();

        // Run in blocking task since RocksDB operations are synchronous
        task::spawn_blocking(move || {
            // Generate embedding
            let embedding = generate_simple_embedding(&text_content);
            
            // Generate UUID
            let id = Uuid::new_v4();
            
            // Store in RocksDB
            semantic.store_context(&id, &text_content, &embedding)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn retrieve_semantic_context(&self, query: &str, k: usize) -> Result<Vec<String>, String> {
        let query_text = query.to_string();
        let semantic = self.semantic.clone();

        task::spawn_blocking(move || {
            // Generate query embedding
            let query_vec = generate_simple_embedding(&query_text);
            
            // Search in RocksDB
            let results = semantic.search_similar(&query_vec, k)
                .map_err(|e| e.to_string())?;
            
            // Extract just the text content
            Ok(results.into_iter().map(|(_, text)| text).collect())
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
