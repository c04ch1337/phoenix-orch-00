use std::env;
use std::path::{Path, PathBuf};

/// Load the Git repository root directory from the `GIT_AGENT_REPO_ROOT`
/// environment variable, validating that it exists and is a directory.
///
/// This function canonicalizes the path to prevent directory traversal and
/// to normalize any `..` components.
pub fn load_repo_root_from_env() -> Result<PathBuf, String> {
    let raw = env::var("GIT_AGENT_REPO_ROOT")
        .map_err(|_| "GIT_AGENT_REPO_ROOT environment variable is not set".to_string())?;

    if raw.trim().is_empty() {
        return Err("GIT_AGENT_REPO_ROOT environment variable is empty".to_string());
    }

    let path = PathBuf::from(raw);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize GIT_AGENT_REPO_ROOT: {e}"))?;

    if !canonical.is_dir() {
        return Err(format!(
            "GIT_AGENT_REPO_ROOT does not point to a directory: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

/// Validate a list of repository-relative paths against a configured
/// repository root. This prevents directory traversal by ensuring that all
/// resolved paths stay within the configured root.
///
/// The input `requested_paths` are treated as paths *relative* to the
/// repository root. Each is joined to `repo_root` and then canonicalized; if
/// any canonical path does not start with `repo_root`, validation fails.
pub fn validate_repo_paths(
    repo_root: &Path,
    requested_paths: &[String],
) -> Result<Vec<PathBuf>, String> {
    let mut validated = Vec::with_capacity(requested_paths.len());

    for raw in requested_paths {
        if raw.trim().is_empty() {
            return Err("Encountered empty path in request; paths must be non-empty".to_string());
        }

        // Treat the incoming path as relative to the repo root. This allows
        // callers to pass paths like `src/lib.rs` or `./src/lib.rs` while
        // still preventing traversal outside the root.
        let candidate = repo_root.join(raw);

        let canonical = candidate
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path '{}': {e}", raw))?;

        if !canonical.starts_with(repo_root) {
            return Err(format!(
                "Path '{}' escapes the configured repository root and is not allowed",
                raw
            ));
        }

        validated.push(canonical);
    }

    Ok(validated)
}
