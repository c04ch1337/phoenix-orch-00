const API_V1_ENDPOINT = 'http://127.0.0.1:8181/api/v1/chat';
const API_LEGACY_ENDPOINT = 'http://127.0.0.1:8181/api/chat';

// Default to v1; we will transparently fall back to legacy if needed.
let API_ENDPOINT = API_V1_ENDPOINT;

// Optional bearer token for minimal auth. This can be injected via a small
// inline script that sets window.ORCH_API_TOKEN, or left unset for local use.
const AUTH_TOKEN = window.ORCH_API_TOKEN || 'dev-token';

const chatContainer = document.getElementById('chat-container');
const messageInput = document.getElementById('message-input');
const sendBtn = document.getElementById('send-btn');
const splashScreen = document.getElementById('splash-screen');
const igniteBtn = document.getElementById('ignite-btn');
const appContainer = document.getElementById('app-container');
const statusDot = document.querySelector('.status-dot');


// Simple UI state machine for basic UX and degraded/error signalling.
const State = Object.freeze({
    Idle: 'idle',
    Loading: 'loading',
    Error: 'error',
    Degraded: 'degraded',
});

let appState = State.Idle;

function setAppState(nextState) {
    appState = nextState;
    document.body.dataset.appState = nextState;
    renderState();
}

function renderState() {
    if (!statusDot) return;

    // Reset classes
    statusDot.classList.remove('state-idle', 'state-loading', 'state-error', 'state-degraded');

    switch (appState) {
        case State.Loading:
            statusDot.classList.add('state-loading');
            statusDot.title = 'Processing request...';
            break;
        case State.Error:
            statusDot.classList.add('state-error');
            statusDot.title = 'Error state - check last message.';
            break;
        case State.Degraded:
            statusDot.classList.add('state-degraded');
            statusDot.title = 'Degraded - using legacy endpoint.';
            break;
        case State.Idle:
        default:
            statusDot.classList.add('state-idle');
            statusDot.title = 'Online';
            break;
    }
}

// Add event listener for ignite button
igniteBtn.addEventListener('click', function () {
    splashScreen.style.display = 'none';
    appContainer.classList.remove('hidden');
    appContainer.removeAttribute('inert');
    setInputState(true); // Ensure input is enabled
    messageInput.focus();
});

// Auto-resize textarea
messageInput.addEventListener('input', function () {
    this.style.height = 'auto';
    this.style.height = (this.scrollHeight) + 'px';
    if (this.value === '') this.style.height = '60px';
});

// Send on Enter (Shift+Enter for new line)
messageInput.addEventListener('keydown', function (e) {
    if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSend();
    }
});

sendBtn.addEventListener('click', handleSend);

async function handleSend() {
    const text = messageInput.value.trim();
    if (!text) return;

    // Add User Message
    appendMessage('User', text, 'user');
    messageInput.value = '';
    messageInput.style.height = '60px';

    // Only disable input if we're not already processing
    if (appState !== State.Loading) {
        setInputState(false);
    }
    setAppState(State.Loading);

    // Add Loading Indicator
    const loadingId = appendMessage('System', 'Thinking...', 'agent', true);

    try {
        const response = await sendMessage(text);

        // Remove Loading Indicator
        removeMessage(loadingId);

        if (response.status === 'success') {
            // Parse the inner JSON string if it exists
            let output = response.output;
            try {
                const parsed = JSON.parse(output);
                if (parsed && typeof parsed === 'object' && parsed.data) {
                    output = parsed.data;
                }
            } catch {
                // Not JSON, use as-is
            }
            appendMessage('Agent', output || '', 'agent');

            if (response.correlation_id) {
                console.info('correlation_id:', response.correlation_id);
            }

            // If we successfully used the v1 endpoint previously marked as degraded,
            // we can return to idle state.
            if (API_ENDPOINT === API_V1_ENDPOINT) {
                setAppState(State.Idle);
            } else {
                // We are using legacy; treat as degraded but successful.
                setAppState(State.Degraded);
            }
        } else {
            appendMessage('Error', response.output || 'Unknown error', 'agent');
            setAppState(State.Error);
        }
    } catch (error) {
        console.error('Error sending message:', error);
        removeMessage(loadingId);
        appendMessage('Error', error && error.message ? error.message : 'Failed to connect to server.', 'agent');
        setAppState(State.Error);
    } finally {
        setInputState(true);
        if (appState === State.Loading) {
            // Ensure we don't get stuck in loading if no state was set above.
            setAppState(State.Idle);
        }
        messageInput.focus();
    }
}

function setSafeContent(contentDiv, text) {
    const safeText = String(text ?? '');
    const parts = safeText.split('\n');

    // Clear any existing children
    while (contentDiv.firstChild) {
        contentDiv.removeChild(contentDiv.firstChild);
    }

    parts.forEach((part, idx) => {
        if (idx > 0) {
            contentDiv.appendChild(document.createElement('br'));
        }
        contentDiv.appendChild(document.createTextNode(part));
    });
}

function appendMessage(role, text, type, isLoading = false) {
    const msgDiv = document.createElement('div');
    msgDiv.className = `message ${type}`;
    const id = 'msg-' + Date.now();
    msgDiv.id = id;

    const roleSpan = document.createElement('span');
    roleSpan.className = 'role';
    roleSpan.textContent = role;

    const contentDiv = document.createElement('div');
    contentDiv.className = 'content';

    // Render content in a DOM-safe way (no innerHTML for user/LLM text).
    setSafeContent(contentDiv, text);

    msgDiv.appendChild(roleSpan);
    msgDiv.appendChild(contentDiv);
    chatContainer.appendChild(msgDiv);

    // Scroll to bottom
    chatContainer.scrollTop = chatContainer.scrollHeight;

    return id;
}

function removeMessage(id) {
    const el = document.getElementById(id);
    if (el) el.remove();
}

function setInputState(enabled) {
    messageInput.disabled = !enabled;
    sendBtn.disabled = !enabled;

    if (enabled) {
        sendBtn.textContent = 'Send';
    } else {
        sendBtn.textContent = '...';
    }
}

async function sendMessage(message) {
    const payload = { message };

    // Helper to perform a POST to a given endpoint and normalize the shape
    // of the response so the rest of the UI can treat both v1 and legacy
    // endpoints uniformly.
    const doPost = async (endpoint) => {
        const headers = {
            'Content-Type': 'application/json',
        };
        if (AUTH_TOKEN) {
            headers['Authorization'] = `Bearer ${AUTH_TOKEN}`;
        }

        const resp = await fetch(endpoint, {
            method: 'POST',
            headers,
            body: JSON.stringify(payload),
        });

        if (!resp.ok) {
            throw new Error(`Request failed (${resp.status} ${resp.statusText})`);
        }

        const data = await resp.json();

        // Heuristically detect v1 vs legacy shapes.
        if (data && typeof data === 'object' && 'api_version' in data) {
            // v1: ChatResponseV1
            return {
                status: data.status,
                output: data.output || '',
                correlation_id: data.correlation_id,
            };
        }

        // Legacy /api/chat shape: { status, output }
        return {
            status: data.status,
            output: data.output,
            correlation_id: undefined,
        };
    };

    // First attempt: v1 endpoint.
    try {
        const normalized = await doPost(API_V1_ENDPOINT);
        API_ENDPOINT = API_V1_ENDPOINT;
        return normalized;
    } catch (e) {
        console.warn('v1 endpoint failed, attempting legacy /api/chat fallback:', e);
    }

    // Fallback: legacy endpoint.
    const normalizedLegacy = await doPost(API_LEGACY_ENDPOINT);
    API_ENDPOINT = API_LEGACY_ENDPOINT;
    return normalizedLegacy;
}

// Expose key functions for Jest tests when running in a Node environment.
if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
        State,
        setAppState,
        renderState,
        setSafeContent,
        appendMessage,
        removeMessage,
        setInputState,
        sendMessage,
        handleSend,
    };
}
