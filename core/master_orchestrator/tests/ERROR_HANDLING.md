# JSON Sanitization and Error Handling Tests

This document explains the test coverage for the JSON sanitization and structured error handling implementation in the master orchestrator.

## Background

Previously, malformed JSON responses could cause "UNKNOWN ERROR" issues where the system would crash instead of handling the error gracefully. The implementation adds proper sanitization and structured error handling to ensure all error conditions are properly captured, reported, and don't cause system failures.

## Test Coverage

### 1. Valid JSON Responses
We verify that normal operation still works correctly with:
- Tests for complete valid JSON responses
- Tests for minimal valid JSON (with only required fields)
- Baseline tests to ensure the success path continues to function

### 2. Malformed JSON Responses
We test various types of JSON malformation that previously caused "UNKNOWN ERROR":
- Missing closing braces (unterminated objects)
- Extra text before/after valid JSON (like log output mixed with JSON)
- Schema violations (missing required fields, incorrect types)
- Completely non-JSON output (plain text errors, empty responses)

### 3. Process Execution Errors
We test agent execution failure scenarios:
- Agents that exit with non-zero status codes
- Agents that timeout
- Successful agents (baseline)

## Key Verification Points

For each error scenario, we verify that:

1. **No crashes occur with malformed input**:
   All error scenarios are caught and translated into proper error responses.

2. **ActionError objects are created with appropriate codes**:
   - 400 (Bad Request) for JSON parsing/validation errors
   - 504 (Gateway Timeout) for agent timeouts
   - 500 (Internal Server Error) for most other errors

3. **Raw output is preserved for debugging**:
   The original output is stored in the `raw_output` field of ActionError.

4. **Error messages are detailed and helpful**:
   Both short user-friendly messages and detailed diagnostic information are provided.

## Testing Approach

To test these scenarios, we used multiple approaches:

1. **Unit Tests for JSON Parsing**:
   Direct tests of the `safe_parse_action_response` function.

2. **Mock Agent Scripts**:
   Command scripts that simulate various error scenarios, including:
   - `failing_agent.cmd` - Always fails with non-zero exit code
   - `hanging_agent.cmd` - Hangs to trigger timeout logic
   - `success_agent.cmd` - Returns valid JSON (baseline)

## Implementation Impact

This implementation ensures that all error scenarios now result in a structured ActionError object rather than crashing with "UNKNOWN ERROR". It provides better diagnostics, improves system stability, and enables more informative error handling throughout the system.