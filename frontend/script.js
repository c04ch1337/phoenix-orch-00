const API_ENDPOINT = 'http://127.0.0.1:8181/api/chat';

const chatContainer = document.getElementById('chat-container');
const messageInput = document.getElementById('message-input');
const sendBtn = document.getElementById('send-btn');
const splashScreen = document.getElementById('splash-screen');
const igniteBtn = document.getElementById('ignite-btn');
const appContainer = document.getElementById('app-container');

// Splash Screen Logic
if (igniteBtn) {
    igniteBtn.addEventListener('click', () => {
        splashScreen.classList.add('fade-out');

        // Wait for transition to finish before showing app fully (optional, or show immediately)
        setTimeout(() => {
            appContainer.classList.remove('hidden');
            // Focus input after transition
            messageInput.focus();
        }, 300); // Start showing app slightly before splash is fully gone for smoother feel
    });
}

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

    // Disable input while processing
    setInputState(false);

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
                if (parsed.data) output = parsed.data;
            } catch (e) {
                // Not JSON, use as is
            }
            appendMessage('Agent', output, 'agent');
        } else {
            appendMessage('Error', response.output || 'Unknown error', 'agent');
        }
    } catch (error) {
        removeMessage(loadingId);
        appendMessage('Error', 'Failed to connect to server.', 'agent');
    } finally {
        setInputState(true);
        messageInput.focus();
    }
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

    // Simple formatting for now (newlines to <br>)
    contentDiv.innerHTML = text.replace(/\n/g, '<br>');

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
    try {
        const response = await fetch(API_ENDPOINT, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ message: message }),
        });
        const data = await response.json();
        return data;
    } catch (error) {
        console.error('Error:', error);
        throw error;
    }
}
