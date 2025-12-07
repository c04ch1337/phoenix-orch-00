describe('Phoenix Orch Core Functionality', () => {
    beforeEach(() => {
        cy.visit('/')
        cy.intercept('POST', '/api/v1/chat').as('chatRequest')
        cy.intercept('POST', '/api/chat').as('legacyChatRequest')
    })

    it('should start with splash screen and transition to main app', () => {
        cy.get('#splash-screen').should('be.visible')
        cy.get('#app-container').should('have.class', 'hidden')

        cy.igniteSystem()

        cy.get('#splash-screen').should('not.be.visible')
        cy.get('#app-container').should('be.visible')
    })

    it('should send messages and receive responses', () => {
        cy.igniteSystem()

        // Send a test message
        cy.sendMessage('test message')

        // Verify message appears in chat
        cy.get('.message.user').last()
            .should('contain', 'test message')

        // Wait for response
        cy.wait('@chatRequest')
        cy.waitForResponse()

        // Verify agent response
        cy.get('.message.agent').last()
            .should('exist')
            .and('not.contain', 'Thinking...')
    })

    it('should handle API fallback gracefully', () => {
        cy.igniteSystem()

        // Force v1 API to fail
        cy.intercept('POST', '/api/v1/chat', {
            statusCode: 500,
            body: 'Server Error'
        }).as('failedRequest')

        cy.sendMessage('test fallback')

        // Should attempt v1 first
        cy.wait('@failedRequest')

        // Then fall back to legacy
        cy.wait('@legacyChatRequest')

        // Status should show degraded
        cy.get('.status-dot')
            .should('have.class', 'state-degraded')

        // But message should still get response
        cy.get('.message.agent').last()
            .should('exist')
            .and('not.contain', 'Thinking...')
    })

    it('should handle network errors appropriately', () => {
        cy.igniteSystem()

        // Force both APIs to fail
        cy.intercept('POST', '/api/v1/chat', {
            statusCode: 500,
            body: 'Server Error'
        })
        cy.intercept('POST', '/api/chat', {
            statusCode: 500,
            body: 'Server Error'
        })

        cy.sendMessage('test error')

        // Status should show error
        cy.get('.status-dot')
            .should('have.class', 'state-error')

        // Error message should appear
        cy.get('.message.agent').last()
            .should('contain', 'Failed to connect to server')
    })

    it('should maintain message history and scroll position', () => {
        cy.igniteSystem()

        // Send multiple messages
        for (let i = 0; i < 5; i++) {
            cy.sendMessage(`test message ${i}`)
            cy.wait('@chatRequest')
            cy.waitForResponse()
        }

        // All messages should exist
        cy.get('.message.user').should('have.length', 5)
        cy.get('.message.agent').should('have.length.at.least', 5)

        // Should auto-scroll to bottom
        cy.get('#chat-container').then($container => {
            const scroll = $container[0].scrollTop
            const height = $container[0].scrollHeight
            expect(scroll).to.be.closeTo(height - $container[0].clientHeight, 1)
        })
    })
})