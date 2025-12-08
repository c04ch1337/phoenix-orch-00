use master_orchestrator::executor::execute_agent;
use shared_types::{ActionRequest, ActionResponse, ApiVersion, Payload};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

// Helper to create a minimal valid action request for testing
fn create_test_request() -> ActionRequest {
    ActionRequest {
        request_id: Uuid::new_v4(),
        api_version: Some(ApiVersion::V1),
        tool: "test_agent".to_string(),
        action: "test_action".to_string(),
        context: "Testing process error handling".to_string(),
        plan_id: None,
        task_id: None,
        correlation_id: None,
        payload: Payload(serde_json::json!({})),
    }
}

// Helper to get absolute path to a mock agent in the tests directory
fn get_mock_agent_path(agent_name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("mock_agents");
    path.push(agent_name);
    path
}

// Test that we properly handle an agent that exits with a non-zero status
#[tokio::test]
async fn test_agent_non_zero_exit_code() {
    let request = create_test_request();
    let timeout = Duration::from_secs(5);
    
    // Replace the executor's path resolution with our mock agent
    let original_path = std::env::var("PATH").unwrap_or_default();
    let mock_dir = get_mock_agent_path("").parent().unwrap().to_string_lossy().to_string();
    let agent_exe = get_mock_agent_path("failing_agent.cmd");
    
    // Execute the mock agent
    let response = execute_agent("failing_agent", &request, timeout).await;
    
    // Verify error handling
    assert_eq!(response.status, "error");
    assert_eq!(response.code, 500);
    assert!(response.result.is_none());
    assert!(response.error.is_some());
    
    let error = response.error.as_ref().unwrap();
    assert_eq!(error.code, 500);
    assert!(error.message.contains("execution failed"), "Error message should indicate execution failure");
    assert!(error.detail.contains("non-zero status"), "Error detail should mention non-zero exit code");
    // Raw output might not be preserved for process execution errors
}

// Test that we properly handle an agent that times out
#[tokio::test]
async fn test_agent_timeout() {
    let request = create_test_request();
    // Use a very short timeout to trigger a timeout condition
    let timeout = Duration::from_millis(100);
    
    // Execute the hanging mock agent
    let response = execute_agent("hanging_agent", &request, timeout).await;
    
    // Verify error handling
    assert_eq!(response.status, "error");
    assert_eq!(response.code, 504); // Gateway Timeout
    assert!(response.result.is_none());
    assert!(response.error.is_some());
    
    let error = response.error.as_ref().unwrap();
    assert_eq!(error.code, 504);
    assert!(error.message.contains("timed out"), "Error message should indicate timeout");
    assert!(error.detail.contains("timed out"), "Error detail should provide timeout information");
}

// Test successful execution (baseline)
#[tokio::test]
async fn test_successful_agent_execution() {
    let request = create_test_request();
    let timeout = Duration::from_secs(5);
    
    // Execute the success mock agent
    let response = execute_agent("success_agent", &request, timeout).await;
    
    // Verify success response
    assert_eq!(response.status, "success");
    assert_eq!(response.code, 0);
    assert!(response.result.is_some());
    assert!(response.error.is_none());
    
    let result = response.result.as_ref().unwrap();
    assert_eq!(result.output_type, "text");
    assert_eq!(result.data, "Test successful");
}

// Additional tests could include:
// - Test agent that can't be found/executed (file not found)
// - Test I/O errors during communication
// - Test agent that produces invalid UTF-8 output