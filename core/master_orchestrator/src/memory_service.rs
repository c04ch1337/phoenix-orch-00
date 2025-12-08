use r2d2;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde_json::Value;
use shared_types::{
    ActionRequest, ActionResponse, AgentError, AgentHealthState, AgentHealthSummaryV1,
    CorrelationId, PlanId, PlanStatus, TaskId, TaskStatus,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::task;
use uuid::Uuid;

use crate::memory::semantic::{generate_simple_embedding, SemanticMemory};

// Type alias for the SQLite connection pool
type DbPool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

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
    pool: Arc<DbPool>,
    semantic: Arc<SemanticMemory>,
}

impl MemoryService {
    pub fn new(db_path: &str, sled_path: &str) -> Result<Self, String> {
        // Create SQLite connection manager
        let manager = SqliteConnectionManager::file(db_path);
        
        // Configure and build the connection pool
        let pool = r2d2::Pool::builder()
            .max_size(10)  // Set appropriate max connections for your workload
            .min_idle(Some(2))  // Keep at least 2 connections ready
            .idle_timeout(Some(Duration::from_secs(300)))  // 5 minute idle timeout
            .max_lifetime(Some(Duration::from_secs(1800)))  // 30 minute max lifetime
            .build(manager)
            .map_err(|e| format!("Failed to create connection pool: {}", e))?;
            
        // Test the pool by getting a connection
        let _ = pool.get().map_err(|e| format!("Failed to get connection from pool: {}", e))?;

        // Initialize Sled for semantic memory
        let semantic =
            SemanticMemory::init(sled_path).map_err(|e| format!("Sled init failed: {}", e))?;

        Ok(Self {
            pool: Arc::new(pool),
            semantic: Arc::new(semantic),
        })
    }

    pub async fn init_gai_memory(&self) -> Result<(), String> {
        let pool = self.pool.clone();
        task::spawn_blocking(move || {
            // Get a connection from the pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;

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
            )
            .map_err(|e| e.to_string())?;

            // Create action_trace_log table
            conn.execute(
                "CREATE TABLE IF NOT EXISTS action_trace_log (
                    trace_id TEXT PRIMARY KEY,
                    request_json TEXT NOT NULL,
                    response_json TEXT NOT NULL,
                    timestamp TEXT DEFAULT CURRENT_TIMESTAMP
                )",
                [],
            )
            .map_err(|e| e.to_string())?;

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
            )
            .map_err(|e| e.to_string())?;

            // Agent health / circuit breaker state.
            conn.execute(
                "CREATE TABLE IF NOT EXISTS agent_health (
                    tool_name TEXT PRIMARY KEY,
                    health TEXT NOT NULL,
                    consecutive_failures INTEGER NOT NULL,
                    last_failure_at TEXT,
                    last_success_at TEXT,
                    circuit_open_until TEXT
                )",
                [],
            )
            .map_err(|e| e.to_string())?;

            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())??;

        // Layer 2: Semantic Memory is now handled by Sled (no flat file dirs needed)
        println!("Semantic Memory (Sled) initialized");

        Ok(())
    }

    pub fn initialize_tool_registry(&self) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
        crate::tool_registry_service::initialize_database(&conn).map_err(|e| e.to_string())
    }

    pub async fn log_action_trace(
        &self,
        request: &ActionRequest,
        response: &ActionResponse,
    ) -> Result<(), String> {
        let pool = self.pool.clone();

        // Clone and redact before persisting or indexing to avoid leaking secrets like api_key
        let mut redacted_request = request.clone();
        redact_secrets(&mut redacted_request.payload.0);

        // Serialize with proper error handling and logging
        let request_json = match serde_json::to_string(&redacted_request) {
            Ok(json) => json,
            Err(err) => {
                tracing::warn!("Failed to serialize action request: {}", err);
                "{}".to_string()
            }
        };
        
        let response_json = match serde_json::to_string(response) {
            Ok(json) => json,
            Err(err) => {
                tracing::warn!("Failed to serialize action response: {}", err);
                "{}".to_string()
            }
        };
        
        let trace_id = request.request_id.to_string();

        // Also store as semantic memory for retrieval (on redacted payload)
        let semantic_text = format!(
            "Action: {} Tool: {} Payload: {}",
            redacted_request.action, redacted_request.tool, redacted_request.payload.0
        );
        self.store_semantic_memory(&semantic_text).await?;

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
  
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
        let pool = self.pool.clone();
        let tool_name = tool_name.to_string();
        let binary_path = binary_path.to_string();
        let description = description.to_string();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
            conn.execute(
                "INSERT INTO agent_registry (tool_name, binary_path, is_active, description)
                 VALUES (?1, ?2, 1, ?3)
                 ON CONFLICT(tool_name) DO UPDATE SET
                    binary_path = excluded.binary_path,
                    is_active = 1,
                    description = excluded.description",
                params![tool_name, binary_path, description],
            )
            .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn get_active_agents(&self) -> Result<Vec<AgentConfig>, String> {
        let pool = self.pool.clone();
        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
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

    pub async fn add_knowledge_triple(
        &self,
        subject: &str,
        predicate: &str,
        object: &str,
    ) -> Result<(), String> {
        let pool = self.pool.clone();
        let s = subject.to_string();
        let p = predicate.to_string();
        let o = object.to_string();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
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
        let pool = self.pool.clone();
        let q = format!("%{}%", query); // Simple LIKE query for now

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
            let mut stmt = conn
                .prepare(
                    "SELECT subject, predicate, object FROM knowledge_graph 
                 WHERE subject LIKE ?1 OR object LIKE ?1 LIMIT 10",
                )
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map(params![q], |row| {
                    let s: String = row.get(0)?;
                    let p: String = row.get(1)?;
                    let o: String = row.get(2)?;
                    Ok(format!("{} {} {}", s, p, o))
                })
                .map_err(|e| e.to_string())?;

            let mut results = Vec::new();
            for row in rows {
                results.push(row.map_err(|e| e.to_string())?);
            }
            Ok::<Vec<String>, String>(results)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    // --- Layer 2: Semantic Memory (Sled) ---

    pub async fn store_semantic_memory(&self, text: &str) -> Result<(), String> {
        let text_content = text.to_string();
        let semantic = self.semantic.clone();

        // Run in blocking task since Sled operations are synchronous
        task::spawn_blocking(move || {
            // Generate embedding
            let embedding = generate_simple_embedding(&text_content);

            // Generate UUID
            let id = Uuid::new_v4();

            // Store in Sled
            semantic
                .store_context(&id, &text_content, &embedding)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn retrieve_semantic_context(
        &self,
        query: &str,
        k: usize,
    ) -> Result<Vec<String>, String> {
        let query_text = query.to_string();
        let semantic = self.semantic.clone();

        task::spawn_blocking(move || {
            // Generate query embedding
            let query_vec = generate_simple_embedding(&query_text);

            // Search in Sled
            let results = semantic
                .search_similar(&query_vec, k)
                .map_err(|e| e.to_string())?;

            // Extract just the text content
            Ok(results.into_iter().map(|(_, text)| text).collect())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn record_plan_state_change(
        &self,
        plan_id: PlanId,
        new_status: PlanStatus,
        description: Option<&str>,
        correlation_id: CorrelationId,
    ) -> Result<(), String> {
        // For now, just log the state change. This can be extended to persist in SQLite/Sled.
        println!(
            "[PLAN_STATE] plan_id={} status={:?} correlation_id={} desc={}",
            plan_id,
            new_status,
            correlation_id,
            description.unwrap_or_default()
        );
        Ok(())
    }

    pub async fn record_task_state_change(
        &self,
        task_id: TaskId,
        plan_id: PlanId,
        new_status: TaskStatus,
        last_error: Option<AgentError>,
        correlation_id: CorrelationId,
    ) -> Result<(), String> {
        // For now, just log the state change. This can be extended to persist in SQLite/Sled.
        if let Some(err) = &last_error {
            println!(
                "[TASK_STATE] task_id={} plan_id={} status={:?} correlation_id={} error_code={:?} error_msg={}",
                task_id,
                plan_id,
                new_status,
                correlation_id,
                err.code,
                err.message
            );
        } else {
            println!(
                "[TASK_STATE] task_id={} plan_id={} status={:?} correlation_id={}",
                task_id, plan_id, new_status, correlation_id
            );
        }
        Ok(())
    }

    /// Mark an agent as healthy after a successful call, resetting its failure count.
    pub async fn update_agent_health_on_success(
        &self,
        tool_name: &str,
        now_iso: &str,
    ) -> Result<(), String> {
        let pool = self.pool.clone();
        let tool_name = tool_name.to_string();
        let now = now_iso.to_string();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
            conn.execute(
                "INSERT INTO agent_health (
                    tool_name, health, consecutive_failures, last_failure_at, last_success_at, circuit_open_until
                ) VALUES (?1, 'healthy', 0, NULL, ?2, NULL)
                ON CONFLICT(tool_name) DO UPDATE SET
                    health = 'healthy',
                    consecutive_failures = 0,
                    last_success_at = excluded.last_success_at,
                    last_failure_at = NULL,
                    circuit_open_until = NULL",
                params![tool_name, now],
            )
            .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Update agent health on failure, applying circuit breaker configuration and
    /// returning the updated health summary.
    pub async fn update_agent_health_on_failure(
        &self,
        tool_name: &str,
        now_iso: &str,
        breaker_cfg: &shared_types::AgentCircuitBreakerConfig,
    ) -> Result<AgentHealthSummaryV1, String> {
        let pool = self.pool.clone();
        let tool_name = tool_name.to_string();
        let now = now_iso.to_string();
        let breaker = breaker_cfg.clone();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;

            // Load existing failure count if present.
            let existing_failures: u32 = match conn.query_row(
                "SELECT consecutive_failures FROM agent_health WHERE tool_name = ?1",
                params![&tool_name],
                |row| row.get(0),
            ) {
                Ok(v) => v,
                Err(_) => 0,
            };

            let new_failures = existing_failures.saturating_add(1);

            // Decide new health state and optional circuit open deadline.
            let (health_str, circuit_open_until): (String, Option<String>) =
                if new_failures >= breaker.failure_threshold {
                    let deadline = (chrono::Utc::now()
                        + chrono::Duration::milliseconds(breaker.cooldown_ms as i64))
                    .to_rfc3339();
                    ("unhealthy".to_string(), Some(deadline))
                } else {
                    ("degraded".to_string(), None)
                };

            conn.execute(
                "INSERT INTO agent_health (
                    tool_name, health, consecutive_failures, last_failure_at, last_success_at, circuit_open_until
                ) VALUES (?1, ?2, ?3, ?4, NULL, ?5)
                ON CONFLICT(tool_name) DO UPDATE SET
                    health = excluded.health,
                    consecutive_failures = excluded.consecutive_failures,
                    last_failure_at = excluded.last_failure_at,
                    circuit_open_until = excluded.circuit_open_until",
                params![
                    &tool_name,
                    &health_str,
                    new_failures as i64,
                    &now,
                    circuit_open_until.as_deref()
                ],
            )
            .map_err(|e| e.to_string())?;

            // Read back the updated row as an AgentHealthSummaryV1.
            let mut stmt = conn
                .prepare(
                    "SELECT tool_name, health, consecutive_failures, last_failure_at, last_success_at, circuit_open_until
                     FROM agent_health
                     WHERE tool_name = ?1",
                )
                .map_err(|e| e.to_string())?;

            let summary = stmt
                .query_row(params![&tool_name], |row| {
                    let agent_id: String = row.get(0)?;
                    let health: String = row.get(1)?;
                    let failures: u32 = row.get(2)?;
                    let last_failure_at: Option<String> = row.get(3)?;
                    let last_success_at: Option<String> = row.get(4)?;
                    let circuit_open_until: Option<String> = row.get(5)?;

                    let health_state = map_health_str(&health);

                    Ok(AgentHealthSummaryV1 {
                        agent_id,
                        health: health_state,
                        consecutive_failures: failures,
                        last_failure_at,
                        last_success_at,
                        circuit_open_until,
                    })
                })
                .map_err(|e| e.to_string())?;

            Ok::<AgentHealthSummaryV1, String>(summary)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Get the current health summary for an agent. If no record exists yet, a
    /// default "healthy" summary is returned.
    pub async fn get_agent_health(&self, tool_name: &str) -> Result<AgentHealthSummaryV1, String> {
        let pool = self.pool.clone();
        let tool_name = tool_name.to_string();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
            let mut stmt = conn
                .prepare(
                    "SELECT tool_name, health, consecutive_failures, last_failure_at, last_success_at, circuit_open_until
                     FROM agent_health
                     WHERE tool_name = ?1",
                )
                .map_err(|e| e.to_string())?;

            let row_result = stmt.query_row(params![&tool_name], |row| {
                let agent_id: String = row.get(0)?;
                let health: String = row.get(1)?;
                let failures: u32 = row.get(2)?;
                let last_failure_at: Option<String> = row.get(3)?;
                let last_success_at: Option<String> = row.get(4)?;
                let circuit_open_until: Option<String> = row.get(5)?;

                let health_state = map_health_str(&health);

                Ok(AgentHealthSummaryV1 {
                    agent_id,
                    health: health_state,
                    consecutive_failures: failures,
                    last_failure_at,
                    last_success_at,
                    circuit_open_until,
                })
            });

            match row_result {
                Ok(summary) => Ok(summary),
                Err(_) => Ok(AgentHealthSummaryV1 {
                    agent_id: tool_name,
                    health: AgentHealthState::Healthy,
                    consecutive_failures: 0,
                    last_failure_at: None,
                    last_success_at: None,
                    circuit_open_until: None,
                }),
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// List health summaries for all known agents.
    pub async fn list_agent_health(&self) -> Result<Vec<AgentHealthSummaryV1>, String> {
        let pool = self.pool.clone();

        task::spawn_blocking(move || {
            // Get connection from pool
            let conn = pool.get().map_err(|e| format!("Failed to get database connection: {}", e))?;
            
            let mut stmt = conn
                .prepare(
                    "SELECT tool_name, health, consecutive_failures, last_failure_at, last_success_at, circuit_open_until
                     FROM agent_health",
                )
                .map_err(|e| e.to_string())?;

            let rows = stmt
                .query_map([], |row| {
                    let agent_id: String = row.get(0)?;
                    let health: String = row.get(1)?;
                    let failures: u32 = row.get(2)?;
                    let last_failure_at: Option<String> = row.get(3)?;
                    let last_success_at: Option<String> = row.get(4)?;
                    let circuit_open_until: Option<String> = row.get(5)?;

                    let health_state = map_health_str(&health);

                    Ok(AgentHealthSummaryV1 {
                        agent_id,
                        health: health_state,
                        consecutive_failures: failures,
                        last_failure_at,
                        last_success_at,
                        circuit_open_until,
                    })
                })
                .map_err(|e| e.to_string())?;

            let mut summaries = Vec::new();
            for row in rows {
                summaries.push(row.map_err(|e| e.to_string())?);
            }
            Ok::<Vec<AgentHealthSummaryV1>, String>(summaries)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Proper shutdown method that flushes pending operations and closes connections
    pub async fn shutdown(&self) {
        // Log shutdown beginning
        println!("[INFO] Beginning memory service shutdown...");
        
        // Flush Sled database
        if let Err(e) = self.semantic.flush() {
            eprintln!("[ERROR] Failed to flush semantic memory: {}", e);
        } else {
            println!("[INFO] Semantic memory flushed successfully");
        }
        
        // Wait for any in-flight operations to complete - give a 2 second timeout
        // This is mainly a placeholder showing the intention, we don't really await them explicitly
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // The connection pool will be dropped when this function completes
        // and the MemoryService is dropped, as it will go out of scope
        println!("[INFO] Memory service shutdown completed");
    }
}

fn map_health_str(s: &str) -> AgentHealthState {
    match s {
        "healthy" => AgentHealthState::Healthy,
        "degraded" => AgentHealthState::Degraded,
        "unhealthy" => AgentHealthState::Unhealthy,
        _ => AgentHealthState::Healthy,
    }
}

/// Recursively redact sensitive fields (e.g., api_key) from JSON values
fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            // Redact common sensitive fields.
            if let Some(v) = map.get_mut("api_key") {
                *v = Value::String("[REDACTED]".to_string());
            }
            for key in &["token", "authorization", "secret"] {
                if let Some(v) = map.get_mut(*key) {
                    *v = Value::String("[REDACTED]".to_string());
                }
            }

            for v in map.values_mut() {
                redact_secrets(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                redact_secrets(v);
            }
        }
        _ => {}
    }
}
