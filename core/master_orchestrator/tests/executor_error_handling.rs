//! Tests for JSON sanitization and error handling in the executor module.
//! These tests verify that we properly handle malformed JSON and other error conditions.

// We need to import the safe_parse_action_response function for direct testing
// This is a bit of a hack, but it allows us to test the internal function directly
use master_orchestrator::executor::{execute_agent, safe_parse_action_response_test_export};
use shared_types::{ActionError, ActionRequest, ActionResponse, ActionResult, ApiVersion, Payload};
use std::time::Duration;
use uuid::Uuid;
use serde_json::json;

// Helper to create a minimal valid action request for testing
fn create_test_request() -> ActionRequest {
    ActionRequest {
        request_id: Uuid::new_v4(),
        api_version: Some(ApiVersion::V1),
        tool: "test_agent".to_string(),
        action: "test_action".to_string(),
        context: "Testing JSON error handling".to_string(),
        plan_id: None,
        task_id: None,
        correlation_id: None,
        payload: Payload(serde_json::json!({})),
    }
}

// Helper to validate error response properties
fn validate_error_response(
    response: &ActionResponse, 
    expected_code: u16,
    code_message: &str,
    has_raw_output: bool
) {
    // Basic response validation
    assert_eq!(response.status, "error");
    assert!(response.code > 0);
    assert!(response.result.is_none());
    assert!(response.error.is_some());
    
    // Error field validation
    let error = response.error.as_ref().unwrap();
    assert_eq!(error.code, expected_code, "Error code should be {}", code_message);
    assert!(!error.message.is_empty(), "Error message should not be empty");
    assert!(!error.detail.is_empty(), "Error detail should not be empty");
    
    // Raw output validation
    if has_raw_output {
        assert!(error.raw_output.is_some(), "Raw output should be preserved");
    }
}

// 1. Valid JSON Response Tests

#[tokio::test]
async fn test_parsing_valid_complete_json() {
    // Valid JSON with all expected fields populated
    let valid_json = r#"{
        "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "api_version": "v1",
        "status": "success",
        "code": 0,
        "result": {
            "output_type": "text",
            "data": "Sample data"
        }
    }"#;
    
    let result = safe_parse_action_response_test_export(valid_json, valid_json.to_string());
    
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, "success");
    assert_eq!(response.code, 0);
    assert!(response.result.is_some());
    let result = response.result.unwrap();
    assert_eq!(result.output_type, "text");
    assert_eq!(result.data, "Sample data");
}

#[tokio::test]
async fn test_parsing_valid_minimal_json() {
    // Minimal valid JSON with only required fields
    let minimal_json = r#"{
        "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "status": "success",
        "code": 0
    }"#;
    
    let result = safe_parse_action_response_test_export(minimal_json, minimal_json.to_string());
    
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.status, "success");
    assert_eq!(response.code, 0);
}

// 2. Malformed JSON Response Tests

#[tokio::test]
async fn test_parsing_missing_closing_braces() {
    // JSON with missing closing brace
    let invalid_json = r#"{
        "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "status": "success",
        "code": 0"#;  // Missing closing brace
    
    let result = safe_parse_action_response_test_export(invalid_json, invalid_json.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
}

#[tokio::test]
async fn test_parsing_extra_text_before_json() {
    // Text before valid JSON
    let text_before = r#"Some log output or error text
        {
            "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
            "status": "success",
            "code": 0
        }"#;
    
    let result = safe_parse_action_response_test_export(text_before, text_before.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
}

#[tokio::test]
async fn test_parsing_extra_text_after_json() {
    // Text after valid JSON
    let text_after = r#"{
            "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
            "status": "success",
            "code": 0
        }
        Some additional text or logging information"#;
    
    let result = safe_parse_action_response_test_export(text_after, text_after.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
}

#[tokio::test]
async fn test_parsing_schema_violations() {
    // Missing required field
    let missing_status = r#"{
        "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "code": 0
    }"#;  // Missing required "status" field
    
    let result = safe_parse_action_response_test_export(missing_status, missing_status.to_string());
    
    assert!(result.is_err());
    let error_resp = result.unwrap_err();
    validate_error_response(&error_resp, 400, "Bad Request", true);
    let error = error_resp.error.as_ref().unwrap();
    assert!(error.detail.contains("schema validation"));
    
    // Incorrect type for field
    let wrong_type = r#"{
        "request_id": "3f2504e0-4f89-41d3-9a0c-0305e82c3301",
        "status": "success",
        "code": "zero"
    }"#;  // "code" should be integer, not string
    
    let result = safe_parse_action_response_test_export(wrong_type, wrong_type.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
}

#[tokio::test]
async fn test_parsing_non_json_output() {
    // Plain text error message
    let plain_text = "Error: Failed to process request";
    
    let result = safe_parse_action_response_test_export(plain_text, plain_text.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
    
    // Empty string
    let empty = "";
    
    let result = safe_parse_action_response_test_export(empty, empty.to_string());
    
    assert!(result.is_err());
    validate_error_response(&result.unwrap_err(), 400, "Bad Request", true);
}

// 3. We need to add code to test process execution errors
// This requires a more complex setup with mock agents

// TODO: Implement tests with mock agents for process execution errors