// Custom commands for Phoenix Orch E2E tests

Cypress.Commands.add('igniteSystem', () => {
    cy.get('#ignite-btn').should('be.visible').click()
    cy.get('#app-container').should('not.have.class', 'hidden')
})

Cypress.Commands.add('sendMessage', (message) => {
    cy.get('#message-input').type(message)
    cy.get('#send-btn').click()
})

Cypress.Commands.add('getLastMessage', () => {
    cy.get('.message').last()
})

Cypress.Commands.add('waitForResponse', () => {
    cy.get('.status-dot')
        .should('have.class', 'state-idle')
        .and('not.have.class', 'state-loading')
})