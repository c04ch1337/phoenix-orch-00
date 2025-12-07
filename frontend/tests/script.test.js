/**
 * Jest tests for frontend script DOM helpers.
 *
 * These tests run in the jsdom environment configured by jest.config.cjs.
 */

describe("frontend script helpers", () => {
    let script;
    let setSafeContent;
    let appendMessage;

    beforeEach(() => {
        // Reset modules so each test gets a clean copy of the script with a fresh DOM.
        jest.resetModules();

        // Provide the minimal DOM expected by script.js at module evaluation time.
        document.body.innerHTML = `
      <div id="splash-screen"></div>
      <button id="ignite-btn">Ignite</button>
      <div id="app-container" class="hidden"></div>
      <div class="status-dot"></div>
      <div id="chat-container"></div>
      <textarea id="message-input"></textarea>
      <button id="send-btn">Send</button>
    `;

        // Require after DOM is set up so the top-level querySelectors succeed.
        // script.js conditionally populates module.exports when running under Node.
        // eslint-disable-next-line global-require
        script = require("../script.js");
        setSafeContent = script.setSafeContent;
        appendMessage = script.appendMessage;
    });

    test("setSafeContent converts newlines to <br> and escapes HTML", () => {
        const div = document.createElement("div");
        const input = "hello\nworld<script>alert('x')</script>";

        setSafeContent(div, input);

        // Ensure newlines are rendered as <br> elements.
        expect(div.innerHTML).toContain("hello");
        expect(div.innerHTML).toContain("<br>");
        expect(div.innerHTML).toContain("world<script>alert('x')</script>");

        // No actual <script> tags should be present in the DOM.
        expect(div.querySelector("script")).toBeNull();
    });

    test("appendMessage creates a message element in the chat container", () => {
        const chatContainer = document.getElementById("chat-container");
        expect(chatContainer).not.toBeNull();

        const id = appendMessage("User", "hi there", "user");
        const msgEl = document.getElementById(id);

        expect(msgEl).not.toBeNull();
        expect(msgEl.classList.contains("message")).toBe(true);
        expect(msgEl.classList.contains("user")).toBe(true);

        const roleSpan = msgEl.querySelector(".role");
        const contentDiv = msgEl.querySelector(".content");

        expect(roleSpan).not.toBeNull();
        expect(roleSpan.textContent).toBe("User");

        expect(contentDiv).not.toBeNull();
        expect(contentDiv.textContent).toBe("hi there");

        // Ensure the new message is appended into the chat container.
        expect(chatContainer.contains(msgEl)).toBe(true);
    });
});