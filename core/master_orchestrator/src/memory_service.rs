use rusqlite::{params, Connection};
use shared_types::{ActionRequest, ActionResponse};
use std::sync::{Arc, Mutex};
use tokio::task;
use std::fs;
use std::path::Path;
use std::io::Write;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
}

impl MemoryService {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
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

        // Layer 2: Semantic Memory (Flat-File Directories)
        fs::create_dir_all("./data/memory/text").map_err(|e| e.to_string())?;
        fs::create_dir_all("./data/memory/vectors").map_err(|e| e.to_string())?;

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

    // --- Layer 2: Semantic Memory (Flat-File Vector) ---

    // 200-Year Longevity: Pure Rust "SimpleHashEmbedding"
    // Maps words to a fixed-size vector (128 dimensions) using hashing.
    // No external dependencies, no models to download, no C++ runtimes.
    fn generate_simple_embedding(text: &str) -> Vec<f32> {
        const DIM: usize = 128;
        let mut vector = vec![0.0; DIM];
        let words = text.split_whitespace();
        
        for word in words {
            let mut hasher = DefaultHasher::new();
            word.hash(&mut hasher);
            let hash = hasher.finish();
            let index = (hash as usize) % DIM;
            vector[index] += 1.0;
        }

        // Normalize
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for x in &mut vector {
                *x /= magnitude;
            }
        }
        
        vector
    }

    pub async fn store_semantic_memory(&self, text: &str) -> Result<(), String> {
        let text_content = text.to_string();

        // 1. Generate Embedding (Pure Rust)
        let embedding = Self::generate_simple_embedding(&text_content);

        // 2. Save to Flat Files
        let id = uuid::Uuid::new_v4().to_string();
        
        // Save Text
        let text_path = format!("./data/memory/text/{}.txt", id);
        fs::write(&text_path, text).map_err(|e| e.to_string())?;

        // Save Vector (Binary)
        let vec_path = format!("./data/memory/vectors/{}.bin", id);
        let mut file = fs::File::create(&vec_path).map_err(|e| e.to_string())?;
        for &val in &embedding {
            let bytes = val.to_le_bytes();
            file.write_all(&bytes).map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    pub async fn retrieve_semantic_context(&self, query: &str, k: usize) -> Result<Vec<String>, String> {
        let query_text = query.to_string();

        // 1. Generate Query Embedding (Pure Rust)
        let query_vec = Self::generate_simple_embedding(&query_text);

        // 2. Scan Flat Files (Linear Search - 200 Year Simplicity)
        let vec_dir = Path::new("./data/memory/vectors");
        let mut scores: Vec<(String, f32)> = Vec::new();

        if vec_dir.exists() {
            for entry in fs::read_dir(vec_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("bin") {
                    let id = path.file_stem().unwrap().to_string_lossy().to_string();
                    
                    // Read vector
                    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                    let mut vector = Vec::new();
                    for chunk in bytes.chunks(4) {
                        if chunk.len() == 4 {
                            let val = f32::from_le_bytes(chunk.try_into().unwrap());
                            vector.push(val);
                        }
                    }

                    // Cosine Similarity
                    let score = cosine_similarity(&query_vec, &vector);
                    scores.push((id, score));
                }
            }
        }

        // 3. Sort and Retrieve Text
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_k = scores.into_iter().take(k).collect::<Vec<_>>();

        let mut results = Vec::new();
        for (id, _score) in top_k {
            let text_path = format!("./data/memory/text/{}.txt", id);
            if let Ok(content) = fs::read_to_string(text_path) {
                results.push(content);
            }
        }

        Ok(results)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}
