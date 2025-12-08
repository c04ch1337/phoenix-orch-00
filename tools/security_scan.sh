#!/bin/bash
# Enhanced security scan script with detailed reporting, CI integration, and remediation advice
set -e

# Output directory for reports
REPORT_DIR="./security_reports"
mkdir -p "$REPORT_DIR"

# Timestamp for reports
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
SUMMARY_FILE="$REPORT_DIR/security_summary_$TIMESTAMP.txt"

echo "🔒 Running comprehensive security scan ($(date))" | tee -a "$SUMMARY_FILE"
echo "=============================================" | tee -a "$SUMMARY_FILE"

# Function to check if a command exists
command_exists() {
  command -v "$1" >/dev/null 2>&1
}

# Install security tools if not present
echo "Installing/updating security tools..." | tee -a "$SUMMARY_FILE"
cargo install cargo-audit --quiet || echo "Warning: Failed to install cargo-audit"
cargo install cargo-deny --quiet || echo "Warning: Failed to install cargo-deny"
cargo install cargo-outdated --quiet || echo "Warning: Failed to install cargo-outdated"

if command_exists trivy; then
    echo "Trivy already installed"
else
    echo "Installing Trivy for container scanning..."
    if command_exists apt-get; then
        sudo apt-get install -y trivy
    elif command_exists brew; then
        brew install trivy
    else
        echo "Warning: Could not install Trivy. Please install manually."
    fi
fi

# Run cargo audit to check for known vulnerabilities
echo -e "\n📋 Running cargo audit..." | tee -a "$SUMMARY_FILE"
AUDIT_REPORT="$REPORT_DIR/cargo_audit_$TIMESTAMP.txt"
if cargo audit --json -q > "$AUDIT_REPORT.json" 2>/dev/null; then
    cargo audit | tee "$AUDIT_REPORT"
    VULN_COUNT=$(grep -c "^Warning:" "$AUDIT_REPORT" || echo "0")
    echo "Found $VULN_COUNT potential vulnerabilities in dependencies" | tee -a "$SUMMARY_FILE"
else
    echo "Warning: cargo audit failed, check environment" | tee -a "$SUMMARY_FILE"
fi

# Run cargo deny to check license compliance and banned dependencies
echo -e "\n📋 Running cargo deny..." | tee -a "$SUMMARY_FILE"
DENY_REPORT="$REPORT_DIR/cargo_deny_$TIMESTAMP.txt"
if cargo deny check licenses bans sources > "$DENY_REPORT" 2>&1; then
    echo "License and banned dependency checks passed" | tee -a "$SUMMARY_FILE"
    cat "$DENY_REPORT" | tail -5 | tee -a "$SUMMARY_FILE"
else
    echo "Warning: Found license or banned dependency issues" | tee -a "$SUMMARY_FILE"
    cat "$DENY_REPORT" | tail -10 | tee -a "$SUMMARY_FILE"
fi

# Check for outdated dependencies
echo -e "\n📋 Checking for outdated dependencies..." | tee -a "$SUMMARY_FILE"
OUTDATED_REPORT="$REPORT_DIR/cargo_outdated_$TIMESTAMP.txt"
cargo outdated > "$OUTDATED_REPORT" 2>&1 || echo "Warning: cargo outdated check failed"
OUTDATED_COUNT=$(grep -c "^[a-z]" "$OUTDATED_REPORT" || echo "0")
echo "Found $OUTDATED_COUNT outdated dependencies" | tee -a "$SUMMARY_FILE"

# Run npm audit for frontend dependencies
echo -e "\n📋 Running npm audit on frontend..." | tee -a "$SUMMARY_FILE"
if [ -d "./frontend" ] && [ -f "./frontend/package.json" ]; then
    NPM_REPORT="$REPORT_DIR/npm_audit_$TIMESTAMP.txt"
    cd frontend
    # Save both human-readable and JSON format
    npm audit > "$NPM_REPORT" 2>&1 || echo "Warning: npm audit found issues"
    npm audit --json > "../$NPM_REPORT.json" 2>/dev/null || echo "Warning: JSON export failed"
    NPM_VULN_COUNT=$(grep -c "vulnerabilities" "$NPM_REPORT" || echo "0")
    echo "Found issues in $NPM_VULN_COUNT npm packages" | tee -a "../$SUMMARY_FILE"
    cd ..
else
    echo "Frontend directory or package.json not found, skipping npm audit" | tee -a "$SUMMARY_FILE"
fi

# Scan Docker image if available
echo -e "\n📋 Scanning Docker image for vulnerabilities..." | tee -a "$SUMMARY_FILE"
if command_exists trivy && command_exists docker; then
    DOCKER_REPORT="$REPORT_DIR/docker_scan_$TIMESTAMP.txt"
    IMAGE_NAME="phoenix-orchestrator:latest"
    # Check if image exists
    if docker image inspect "$IMAGE_NAME" > /dev/null 2>&1; then
        trivy image --severity HIGH,CRITICAL "$IMAGE_NAME" > "$DOCKER_REPORT" 2>&1 || echo "Issues found in container image"
        CONTAINER_VULN_COUNT=$(grep -c "VULNERABILITY ID" "$DOCKER_REPORT" || echo "0")
        echo "Found $CONTAINER_VULN_COUNT container vulnerabilities" | tee -a "$SUMMARY_FILE"
    else
        echo "Docker image not found locally, skipping container scan" | tee -a "$SUMMARY_FILE"
    fi
else
    echo "Trivy or Docker not available, skipping container scan" | tee -a "$SUMMARY_FILE"
fi

# Enhanced custom security checks
echo -e "\n📋 Running custom security checks..." | tee -a "$SUMMARY_FILE"
CUSTOM_REPORT="$REPORT_DIR/custom_checks_$TIMESTAMP.txt"

# Check for hardcoded secrets with more advanced pattern matching
echo "Checking for potential hardcoded secrets..." | tee -a "$CUSTOM_REPORT"
PATTERNS="password\|secret\|token\|key\|apikey\|api_key\|pwd\|credentials"
FILES="*.rs *.js *.json *.toml *.yaml *.yml *.env* *.sh"

git grep -l -i "$PATTERNS" -- $FILES 2>/dev/null | \
while read file; do
    echo "⚠️ Potential sensitive data in: $file" | tee -a "$CUSTOM_REPORT"
    # Show the context for each match (but mask the actual values)
    git grep -i -n "$PATTERNS" "$file" | sed 's/\(.*\)\(password\|secret\|token\|key\).*=/\1\2=*** REDACTED ***/i' | tee -a "$CUSTOM_REPORT"
done

# Check for unsafe Rust code usage with line numbers
echo -e "\nChecking for unsafe code usage..." | tee -a "$CUSTOM_REPORT"
UNSAFE_COUNT=0
find . -name "*.rs" -type f -not -path "./target/*" | \
while read file; do
    UNSAFE_LINES=$(grep -n "unsafe" "$file" || true)
    if [ ! -z "$UNSAFE_LINES" ]; then
        echo "⚠️ Unsafe code in $file:" | tee -a "$CUSTOM_REPORT"
        echo "$UNSAFE_LINES" | tee -a "$CUSTOM_REPORT"
        UNSAFE_COUNT=$((UNSAFE_COUNT + 1))
    fi
done
echo "Found unsafe code in approximately $UNSAFE_COUNT files" | tee -a "$SUMMARY_FILE"

# Check for debug assertions in production code
echo -e "\nChecking for debug assertions..." | tee -a "$CUSTOM_REPORT"
DEBUG_COUNT=0
find . -name "*.rs" -type f -not -path "./target/*" | \
while read file; do
    DEBUG_LINES=$(grep -n "debug_assert" "$file" || true)
    if [ ! -z "$DEBUG_LINES" ]; then
        echo "⚠️ Debug assertions in $file:" | tee -a "$CUSTOM_REPORT"
        echo "$DEBUG_LINES" | tee -a "$CUSTOM_REPORT"
        DEBUG_COUNT=$((DEBUG_COUNT + 1))
    fi
done
echo "Found debug assertions in approximately $DEBUG_COUNT files" | tee -a "$SUMMARY_FILE"

# Check for unwrap calls that could fail in production
echo -e "\nChecking for unwrap() calls (potential panics)..." | tee -a "$CUSTOM_REPORT"
UNWRAP_COUNT=0
find . -name "*.rs" -type f -not -path "./target/*" | \
while read file; do
    UNWRAP_LINES=$(grep -n -v "fn " "$file" | grep -n "unwrap()" || true)
    if [ ! -z "$UNWRAP_LINES" ]; then
        echo "⚠️ Potential panic points in $file:" | tee -a "$CUSTOM_REPORT"
        echo "$UNWRAP_LINES" | tee -a "$CUSTOM_REPORT"
        UNWRAP_COUNT=$((UNWRAP_COUNT + 1))
    fi
done
echo "Found unwrap() calls in approximately $UNWRAP_COUNT files" | tee -a "$SUMMARY_FILE"

# Check for temporary directories that aren't being cleaned up
echo -e "\nChecking for potential temp file issues..." | tee -a "$CUSTOM_REPORT"
find . -name "*.rs" -type f -not -path "./target/*" | xargs grep -l "std::env::temp_dir" > "$REPORT_DIR/temp_files.txt" || true
TEMP_COUNT=$(wc -l "$REPORT_DIR/temp_files.txt" | awk '{print $1}')
echo "Found $TEMP_COUNT files using temporary directories" | tee -a "$SUMMARY_FILE"

# Check .env file security
if [ -f ".env" ]; then
    echo -e "\nChecking .env file permissions..." | tee -a "$CUSTOM_REPORT"
    if [ "$(stat -c %a .env)" != "600" ]; then
        echo "⚠️ .env file has insecure permissions! Should be 600, current: $(stat -c %a .env)" | tee -a "$CUSTOM_REPORT" | tee -a "$SUMMARY_FILE"
        echo "Run: chmod 600 .env" | tee -a "$CUSTOM_REPORT"
    else
        echo "✅ .env file has secure permissions" | tee -a "$CUSTOM_REPORT"
    fi
fi

# Check for debug/development configurations in production files
echo -e "\nChecking for debug configurations in production files..." | tee -a "$CUSTOM_REPORT"
find . -type f -name "*prod*.toml" -o -name "*prod*.json" | \
while read file; do
    DEBUG_CONFIG=$(grep -i "debug\|dev\|test" "$file" || true)
    if [ ! -z "$DEBUG_CONFIG" ]; then
        echo "⚠️ Potential debug settings in production file $file:" | tee -a "$CUSTOM_REPORT"
        echo "$DEBUG_CONFIG" | tee -a "$CUSTOM_REPORT"
    fi
done

# Summarize findings
echo -e "\n📊 Security Scan Summary" | tee -a "$SUMMARY_FILE"
echo "======================" | tee -a "$SUMMARY_FILE"
echo "Reports saved to: $REPORT_DIR" | tee -a "$SUMMARY_FILE"
echo -e "Scan completed at: $(date)" | tee -a "$SUMMARY_FILE"

# Copy summary to latest.txt for CI integration
cp "$SUMMARY_FILE" "$REPORT_DIR/latest_scan_summary.txt"

echo -e "\n✅ Security scan complete!"
echo "Full reports available in: $REPORT_DIR"