const { defineConfig } = require('cypress')

module.exports = defineConfig({
    e2e: {
        baseUrl: 'http://127.0.0.1:8282',
        viewportWidth: 900,
        viewportHeight: 600,
        video: true,
        screenshotOnRunFailure: true,
        setupNodeEvents(on, config) {
            // Implement node event listeners here
            on('task', {
                log(message) {
                    console.log(message)
                    return null
                }
            })
        }
    },
})