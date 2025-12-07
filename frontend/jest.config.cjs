module.exports = {
    testEnvironment: "jsdom",
    roots: ["<rootDir>/tests"],
    moduleFileExtensions: ["js", "jsx", "json"],
    collectCoverage: true,
    coverageReporters: ["lcov", "text", "html"],
    coverageDirectory: "coverage",
    coverageThreshold: {
        global: {
            branches: 80,
            functions: 80,
            lines: 80,
            statements: 80
        }
    },
    coveragePathIgnorePatterns: [
        "/node_modules/",
        "/tests/",
        "/dist/"
    ],
    testMatch: [
        "<rootDir>/tests/**/*.test.js"
    ],
    setupFilesAfterEnv: [
        "<rootDir>/tests/setup.js"
    ]
};