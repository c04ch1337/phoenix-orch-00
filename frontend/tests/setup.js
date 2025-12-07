// Jest setup file for frontend tests

// Mock browser APIs not available in jsdom
global.ResizeObserver = class ResizeObserver {
    observe() { }
    unobserve() { }
    disconnect() { }
};

// Mock WebSocket for chat functionality testing
global.WebSocket = class WebSocket {
    constructor(url) {
        this.url = url;
        this.readyState = WebSocket.CONNECTING;
        setTimeout(() => {
            this.readyState = WebSocket.OPEN;
            if (this.onopen) this.onopen();
        }, 0);
    }

    send(data) {
        if (this.readyState !== WebSocket.OPEN) {
            throw new Error('WebSocket is not open');
        }
        // Simulate response
        setTimeout(() => {
            if (this.onmessage) {
                this.onmessage({ data: JSON.stringify({ status: 'success' }) });
            }
        }, 50);
    }

    close() {
        this.readyState = WebSocket.CLOSED;
        if (this.onclose) this.onclose();
    }

    static get CONNECTING() { return 0; }
    static get OPEN() { return 1; }
    static get CLOSING() { return 2; }
    static get CLOSED() { return 3; }
};

// Mock fetch API
global.fetch = jest.fn(() =>
    Promise.resolve({
        ok: true,
        json: () => Promise.resolve({ status: 'success' }),
    })
);