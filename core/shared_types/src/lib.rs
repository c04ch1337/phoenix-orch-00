use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

// Consciousness layer types for AGI personality
pub mod consciousness;
pub use consciousness::*;

/// Arbitrary structured payload, whose schema depends on the agent and action.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Payload(pub Value);

/// API version for all external and agent-facing contracts.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiVersion {
    V0,
    V1,
}

/// Identifier types shared across orchestrator, agents, and clients.
pub type PlanId = uuid::Uuid;
pub type TaskId = uuid::Uuid;

/// Logical identifier for an agent implementation (e.g. "llm_router_agent").
pub type AgentId = String;

/// Correlation identifier used to join logs/traces across orchestrator and agents.
pub type CorrelationId = uuid::Uuid;

/// Convenience constant for current stable version.
pub const API_VERSION_CURRENT: ApiVersion = ApiVersion::V1;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionRequest {
    /// Unique ID for this specific agent invocation.
    pub request_id: Uuid,

    /// Optional protocol version. If omitted, the orchestrator treats this as V0.
    #[serde(default)]
    pub api_version: Option<ApiVersion>,

    /// Logical agent identifier (e.g. "llm_router_agent", "git_agent").
    pub tool: String,

    /// Logical action name within the agent (e.g. "execute", "git_status").
    pub action: String,

    /// High-level natural-language or contextual description.
    pub context: String,

    /// Optional plan this action belongs to.
    #[serde(default)]
    pub plan_id: Option<PlanId>,

    /// Optional task this action belongs to.
    #[serde(default)]
    pub task_id: Option<TaskId>,

    /// Correlation identifier propagated from external caller through orchestrator.
    #[serde(default)]
    pub correlation_id: Option<CorrelationId>,

    /// Arbitrary structured payload, whose schema depends on the agent and action.
    pub payload: Payload,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionResult {
    pub output_type: String,
    pub data: String,
    pub metadata: Option<serde_json::Value>,
}

/// Structured error response for agent invocations.
/// Used to provide detailed error information and assist in error handling and diagnosis.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionError {
    /// Error code for categorization (non-zero)
    pub code: u16,
    
    /// Short user-friendly error summary
    pub message: String,
    
    /// Full diagnostic message with detailed error information
    pub detail: String,
    
    /// The raw, unparsed STDOUT from the agent (if available)
    pub raw_output: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ActionResponse {
    /// Echo of the request_id provided in ActionRequest.
    pub request_id: Uuid,

    /// Optional protocol version used for this response.
    #[serde(default)]
    pub api_version: Option<ApiVersion>,

    /// Status of the agent invocation (e.g. "success", "error").
    pub status: String,

    /// Agent-specific numeric code (0 for success, non-zero for error).
    pub code: u16,

    /// Result payload when status == "success".
    pub result: Option<ActionResult>,

    /// Structured error information when status != "success".
    /// Replaces the previous string-only error field with a more comprehensive structure.
    pub error: Option<ActionError>,

    /// Optional plan this response belongs to.
    #[serde(default)]
    pub plan_id: Option<PlanId>,

    /// Optional task this response belongs to.
    #[serde(default)]
    pub task_id: Option<TaskId>,

    /// Correlation identifier propagated from the request.
    #[serde(default)]
    pub correlation_id: Option<CorrelationId>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: String,
    pub max_input_tokens: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LLMConfig {
    pub default_provider: String,
    pub openrouter: Option<ProviderConfig>,
    pub gemini: Option<ProviderConfig>,
    pub grok: Option<ProviderConfig>,
    pub openai: Option<ProviderConfig>,
    pub anthropic: Option<ProviderConfig>,
    pub ollama: Option<ProviderConfig>,
    pub lmstudio: Option<ProviderConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentRetryConfig {
    pub max_attempts: u8,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentCircuitBreakerConfig {
    pub failure_threshold: u32,
    pub cooldown_ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentExecutionConfig {
    pub timeout_secs: u64,
    pub retry: AgentRetryConfig,
    pub circuit_breaker: AgentCircuitBreakerConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentsConfig {
    pub default: AgentExecutionConfig,
    pub git_agent: Option<AgentExecutionConfig>,
    pub obsidian_agent: Option<AgentExecutionConfig>,
    pub llm_router_agent: Option<AgentExecutionConfig>,
}

/// Redis cache configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RedisConfig {
    /// Redis server URL (e.g., "redis://127.0.0.1:6379")
    pub url: String,
    /// Redis connection pool size
    pub pool_size: u32,
    /// Default TTL for cache entries in seconds
    pub ttl_seconds: u64,
    /// Connection timeout in milliseconds
    #[serde(default)]
    pub connection_timeout_ms: Option<u64>,
    /// Maximum retry attempts for Redis operations
    #[serde(default)]
    pub max_retries: Option<u32>,
    /// Delay between retry attempts in milliseconds
    #[serde(default)]
    pub retry_delay_ms: Option<u64>,
    /// Read timeout in milliseconds
    #[serde(default)]
    pub read_timeout_ms: Option<u64>,
    /// Write timeout in milliseconds
    #[serde(default)]
    pub write_timeout_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub llm: LLMConfig,
    #[serde(default)]
    pub agents: Option<AgentsConfig>,
    /// Redis caching configuration, if enabled
    #[serde(default)]
    pub redis: Option<RedisConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub version: String,
    pub description: String,
    pub executable_path: String,
    pub actions_schema: serde_json::Value,
    pub tags: String,
    pub category: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum ToolError {
    IOError(String),
    SerializationError(String),
    DeserializationError(String),
    ExecutionError(String),
    Timeout(String),
    InvalidAgentResponse(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::IOError(msg) => write!(f, "IO error: {}", msg),
            ToolError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ToolError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
            ToolError::ExecutionError(msg) => write!(f, "Execution error: {}", msg),
            ToolError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            ToolError::InvalidAgentResponse(msg) => write!(f, "Invalid agent response: {}", msg),
        }
    }
}

impl std::error::Error for ToolError {}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum OrchestratorErrorCode {
    ValidationFailed,
    PlanningFailed,
    ExecutionFailed,
    AgentUnavailable,
    Timeout,
    Internal,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorCode {
    InvalidRequest,
    ActionNotSupported,
    BackendFailure,
    Timeout,
    Io,
    Internal,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OrchestratorError {
    pub code: OrchestratorErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentError {
    pub code: AgentErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Dispatched,
    InProgress,
    Succeeded,
    Failed,
    Retried,
    DeadLettered,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentHealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentHealthSummaryV1 {
    pub agent_id: AgentId,
    pub health: AgentHealthState,
    pub consecutive_failures: u32,
    pub last_failure_at: Option<String>,
    pub last_success_at: Option<String>,
    pub circuit_open_until: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlanSummaryV1 {
    pub id: PlanId,
    pub status: PlanStatus,
    pub created_at: String,
    pub updated_at: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TaskSummaryV1 {
    pub id: TaskId,
    pub plan_id: PlanId,
    pub agent_id: AgentId,
    pub status: TaskStatus,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<AgentError>,
    #[serde(default)]
    pub agent_health: Option<AgentHealthState>,
}

fn default_api_version() -> ApiVersion {
    API_VERSION_CURRENT
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatRequestV1 {
    #[serde(default = "default_api_version")]
    pub api_version: ApiVersion,
    pub correlation_id: Option<CorrelationId>,
    pub message: String,
    pub context: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatResponseV1 {
    pub api_version: ApiVersion,
    pub correlation_id: CorrelationId,
    pub status: String,
    pub plan_id: Option<PlanId>,
    pub output: Option<String>,
    pub error: Option<OrchestratorError>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientToOrchestratorWsMessageV1 {
    Chat(ChatRequestV1),
    SubscribePlan {
        plan_id: PlanId,
        #[serde(default)]
        last_event_id: Option<String>,
    },
    UnsubscribePlan {
        plan_id: PlanId,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum OrchestratorToClientWsMessageV1 {
    ChatResult(ChatResponseV1),
    PlanUpdated {
        correlation_id: CorrelationId,
        plan: PlanSummaryV1,
    },
    TaskUpdated {
        correlation_id: CorrelationId,
        task: TaskSummaryV1,
    },
    Error {
        correlation_id: CorrelationId,
        error: OrchestratorError,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRouterConfigV1 {
    pub provider: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LlmRouterInvocationRequestV1 {
    pub prompt: String,
    pub config: LlmRouterConfigV1,
}

fn default_git_log_limit() -> u32 {
    10
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GitAgentCommandV1 {
    GitStatus,
    GitDiff {
        files: Vec<String>,
    },
    GitLog {
        #[serde(default = "default_git_log_limit")]
        limit: u32,
    },
    GitAdd {
        files: Vec<String>,
    },
    GitCommit {
        message: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GitAgentRequestV1 {
    pub command: GitAgentCommandV1,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObsidianAgentCommandV1 {
    CreateNote {
        vault_path: String,
        note_name: String,
        content: String,
    },
    ReadNote {
        vault_path: String,
        note_name: String,
    },
    UpdateNote {
        vault_path: String,
        note_name: String,
        content: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ObsidianAgentRequestV1 {
    pub command: ObsidianAgentCommandV1,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chat_request_v1_defaults_api_version() {
        // JSON payload omits `api_version` so it should default to API_VERSION_CURRENT.
        let value = json!({
            "message": "hello world",
            "correlation_id": null,
            "context": null
        });

        let req: ChatRequestV1 =
            serde_json::from_value(value).expect("deserialization should succeed");
        assert_eq!(req.api_version, API_VERSION_CURRENT);
        assert_eq!(req.message, "hello world");
    }

    #[test]
    fn action_response_round_trip() {
        let original = ActionResponse {
            request_id: Uuid::new_v4(),
            api_version: Some(ApiVersion::V1),
            status: "success".to_string(),
            code: 0,
            result: Some(ActionResult {
                output_type: "text".to_string(),
                data: "result data".to_string(),
                metadata: Some(json!({ "foo": "bar" })),
            }),
            error: None,
            plan_id: Some(Uuid::new_v4()),
            task_id: Some(Uuid::new_v4()),
            correlation_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let decoded: ActionResponse =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(decoded.status, original.status);
        assert_eq!(decoded.code, original.code);
        assert!(decoded.result.is_some());
        assert_eq!(
            decoded.result.as_ref().unwrap().data,
            original.result.as_ref().unwrap().data
        );
    }
}

    #[test]
    fn action_error_serialization() {
        let error = ActionError {
            code: 500,
            message: "Processing failed".to_string(),
            detail: "The agent encountered an unexpected error while processing the request".to_string(),
            raw_output: Some("Error: Cannot parse response at line 42".to_string()),
        };

        let json = serde_json::to_string(&error).expect("serialization should succeed");
        let decoded: ActionError = serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(decoded.code, error.code);
        assert_eq!(decoded.message, error.message);
        assert_eq!(decoded.detail, error.detail);
        assert_eq!(decoded.raw_output, error.raw_output);
    }

    #[test]
    fn action_response_with_error() {
        let original = ActionResponse {
            request_id: Uuid::new_v4(),
            api_version: Some(ApiVersion::V1),
            status: "error".to_string(),
            code: 500,
            result: None,
            error: Some(ActionError {
                code: 500,
                message: "JSON parsing failed".to_string(),
                detail: "Malformed JSON received from agent".to_string(),
                raw_output: Some("Invalid JSON: unexpected token at line 3".to_string()),
            }),
            plan_id: Some(Uuid::new_v4()),
            task_id: Some(Uuid::new_v4()),
            correlation_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let decoded: ActionResponse = serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(decoded.status, original.status);
        assert_eq!(decoded.code, original.code);
        assert!(decoded.error.is_some());
        let error = decoded.error.as_ref().unwrap();
        let original_error = original.error.as_ref().unwrap();
        assert_eq!(error.code, original_error.code);
        assert_eq!(error.message, original_error.message);
        assert_eq!(error.detail, original_error.detail);
        assert_eq!(error.raw_output, original_error.raw_output);
    }
