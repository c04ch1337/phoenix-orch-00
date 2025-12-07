#!/bin/bash
set -e

echo "🔒 Running security scans..."

# Install security tools if not present
echo "Installing/updating security tools..."
cargo install cargo-audit --quiet || true
cargo install cargo-deny --quiet || true
cargo install cargo-outdated --quiet || true

# Run cargo audit to check for known vulnerabilities
echo -e "\n📋 Running cargo audit..."
cargo audit

# Run cargo deny to check license compliance and banned dependencies
echo -e "\n📋 Running cargo deny..."
cargo deny check licenses
cargo deny check bans
cargo deny check sources

# Check for outdated dependencies
echo -e "\n📋 Checking for outdated dependencies..."
cargo outdated

# Run npm audit for frontend dependencies
echo -e "\n📋 Running npm audit on frontend..."
cd frontend
npm audit
cd ..

# Custom security checks
echo -e "\n📋 Running custom security checks..."

# Check for hardcoded secrets
echo "Checking for potential hardcoded secrets..."
git grep -l "password\|secret\|token\|key" -- "*.rs" "*.js" "*.json" "*.toml" | \
while read file; do
    echo "Potential sensitive data in: $file"
done

# Check for unsafe Rust code usage
echo "Checking for unsafe code usage..."
find . -name "*.rs" -type f -exec grep -l "unsafe" {} \;

# Check for debug/development configurations in production files
echo "Checking for debug configurations..."
find . -type f -name "*.toml" -o -name "*.json" | \
while read file; do
    grep -l "debug\|dev\|test" "$file" || true
done

echo -e "\n✅ Security scan complete!"