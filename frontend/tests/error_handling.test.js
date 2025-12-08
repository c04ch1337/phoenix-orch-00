/**
 * Tests for frontend error handling and rendering
 * 
 * This test suite specifically verifies that our frontend correctly handles
 * different types of error responses from the API, ensuring proper error
 * rendering and user-friendly display of technical details.
 */

describe("Frontend error handling", () => {
    let script;
    let appendMessage;
    let chatContainer;

    // Test cases for different error scenarios
    const testCases = [
        {
            id: "case1",
            name: "JSON Parsing Error",
            description: "Malformed JSON that can't be parsed properly",
            response: {
                status: "error",
                output: "{ \"data\": \"this is broken JSON with missing closings",
                error: null
            },
            expected: {
                message: '{ "data": "this is broken JSON with missing closings',
                hasDetails: false
            }
        },
        {
            id: "case2",
            name: "Process Execution Error",
            description: "Agent process crashed or timed out",
            response: {
                status: "error",
                code: 500,
                output: null,
                error: {
                    code: 500,
                    message: "Agent execution failed",
                    detail: "Process exited with non-zero status code 1. The command may not exist or might have crashed during execution.",
                    raw_output: "Error: Command not found\nStack trace:\n  at Process.ChildProcess._handle.onexit (internal/child_process.js:290:12)"
                }
            },
            expected: {
                message: "Agent execution failed",
                hasDetails: true,
                detailText: "Process exited with non-zero status code 1. The command may not exist or might have crashed during execution."
            }
        },
        {
            id: "case3",
            name: "ActionError with Details",
            description: "Well-structured error with message and technical details",
            response: {
                status: "error",
                code: 400,
                output: null,
                error: {
                    code: 400,
                    message: "Failed to process the command due to invalid syntax",
                    detail: "The command 'git pull' failed because the repository is not initialized. Try running 'git init' first or check if you're in the correct directory."
                }
            },
            expected: {
                message: "Failed to process the command due to invalid syntax",
                hasDetails: true,
                detailText: "The command 'git pull' failed because the repository is not initialized. Try running 'git init' first or check if you're in the correct directory."
            }
        },
        {
            id: "case4",
            name: "Legacy Error String",
            description: "Old error format with just a string message",
            response: {
                status: "error",
                output: "Error: Operation failed due to an unexpected condition",
                error: null
            },
            expected: {
                message: "Error: Operation failed due to an unexpected condition",
                hasDetails: false
            }
        },
        {
            id: "case5",
            name: "Empty Error Response",
            description: "Error without any useful information",
            response: {
                status: "error",
                output: "",
                error: null
            },
            expected: {
                message: "Unknown error",
                hasDetails: false
            }
        },
        {
            id: "case6",
            name: "Nested JSON Error with Context",
            description: "Error with additional contextual information",
            response: {
                status: "error",
                code: 422,
                output: null,
                error: {
                    code: 422,
                    message: "Command validation failed",
                    detail: "The provided command parameters don't match the expected format",
                    context: {
                        received: { format: "jpg", size: "large" },
                        expected: { format: ["png", "webp"], size: ["small", "medium"] }
                    }
                }
            },
            expected: {
                message: "Command validation failed",
                hasDetails: true,
                detailText: "The provided command parameters don't match the expected format"
            }
        }
    ];

    beforeEach(() => {
        // Reset modules so each test gets a clean copy of the script with a fresh DOM
        jest.resetModules();

        // Provide the minimal DOM expected by script.js at module evaluation time
        document.body.innerHTML = `
      <div id="splash-screen"></div>
      <button id="ignite-btn">Ignite</button>
      <div id="app-container" class="hidden"></div>
      <div class="status-dot"></div>
      <div id="chat-container"></div>
      <textarea id="message-input"></textarea>
      <button id="send-btn">Send</button>
    `;

        // Require after DOM is set up so the top-level querySelectors succeed
        script = require("../script.js");
        appendMessage = script.appendMessage;
        chatContainer = document.getElementById("chat-container");
    });

    test("Error rendering for various error types", () => {
        // For each test case, check if error is rendered correctly
        testCases.forEach((testCase) => {
            console.log(`Testing ${testCase.name}...`);

            // Clear chat container
            while (chatContainer.firstChild) {
                chatContainer.removeChild(chatContainer.firstChild);
            }

            // Handle response
            if (testCase.response.status === "error") {
                if (testCase.response.error && typeof testCase.response.error === "object") {
                    const errorObj = testCase.response.error;

                    // Handle structured error objects
                    const errorMessage = errorObj.message || "An error occurred";
                    const errorDetail = errorObj.detail ||
                        (errorObj.details ? JSON.stringify(errorObj.details, null, 2) : null) ||
                        "No additional details available";

                    // Show error with expandable technical details
                    appendMessage("Error", errorMessage, "agent", false, true, errorDetail);
                } else {
                    // Fallback to previous behavior for backward compatibility
                    appendMessage("Error", testCase.response.output || "Unknown error", "agent");
                }
            }

            // Verify rendering
            const messageElements = chatContainer.querySelectorAll(".message");
            expect(messageElements.length).toBe(1);

            const messageContent = messageElements[0].querySelector(".content");
            expect(messageContent).not.toBeNull();

            // Verify message content
            expect(messageContent.textContent).toContain(testCase.expected.message);

            // Verify details button and container for errors that should have details
            const detailsBtn = messageContent.querySelector(".error-details-btn");
            if (testCase.expected.hasDetails) {
                expect(detailsBtn).not.toBeNull();

                // Verify details container exists and contains the expected text
                const detailsContainer = messageContent.querySelector(".error-details");
                expect(detailsContainer).not.toBeNull();
                expect(detailsContainer.textContent).toContain(testCase.expected.detailText);

                // Initially hidden
                expect(detailsContainer.style.display).toBe("none");

                // Simulate clicking the details button
                detailsBtn.click();

                // Should now be visible
                expect(detailsContainer.style.display).toBe("block");
            } else {
                // No details button for errors without details
                expect(detailsBtn).toBeNull();
            }
        });
    });

    test("Should never display 'UNKNOWN ERROR' when structured data is available", () => {
        // Focus on the structured error cases
        const structuredErrorCases = testCases.filter(tc =>
            tc.response.error && typeof tc.response.error === "object");

        structuredErrorCases.forEach(testCase => {
            // Clear chat container
            while (chatContainer.firstChild) {
                chatContainer.removeChild(chatContainer.firstChild);
            }

            // Handle response
            const errorObj = testCase.response.error;
            const errorMessage = errorObj.message || "An error occurred";
            const errorDetail = errorObj.detail ||
                (errorObj.details ? JSON.stringify(errorObj.details, null, 2) : null) ||
                "No additional details available";

            appendMessage("Error", errorMessage, "agent", false, true, errorDetail);

            // Check that error message is displayed properly
            const messageElements = chatContainer.querySelectorAll(".message");
            const messageContent = messageElements[0].querySelector(".content");

            // Should never display "Unknown error" when we have structured data
            expect(messageContent.textContent).not.toContain("Unknown error");
            expect(messageContent.textContent).toContain(errorObj.message);
        });
    });

    test("Shows 'Unknown error' only as a last resort", () => {
        // Empty error response
        const emptyErrorCase = testCases.find(tc => tc.id === "case5");

        // Clear chat container
        while (chatContainer.firstChild) {
            chatContainer.removeChild(chatContainer.firstChild);
        }

        // Handle completely empty error response
        appendMessage("Error", emptyErrorCase.response.output || "Unknown error", "agent");

        // Check that "Unknown error" is displayed
        const messageElements = chatContainer.querySelectorAll(".message");
        const messageContent = messageElements[0].querySelector(".content");
        expect(messageContent.textContent).toContain("Unknown error");
    });
});