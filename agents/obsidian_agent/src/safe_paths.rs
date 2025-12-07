use std::env;
use std::path::{Path, PathBuf};

/// Load the Obsidian vault root directory from the `OBSIDIAN_AGENT_VAULT_ROOT`
/// environment variable, validating that it exists and is a directory.
///
/// The resulting path is canonicalized to prevent directory traversal and
/// normalize any `..` components.
pub fn load_vault_root_from_env() -> Result<PathBuf, String> {
    let raw = env::var("OBSIDIAN_AGENT_VAULT_ROOT")
        .map_err(|_| "OBSIDIAN_AGENT_VAULT_ROOT environment variable is not set".to_string())?;

    if raw.trim().is_empty() {
        return Err("OBSIDIAN_AGENT_VAULT_ROOT environment variable is empty".to_string());
    }

    let path = PathBuf::from(raw);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize OBSIDIAN_AGENT_VAULT_ROOT: {e}"))?;

    if !canonical.is_dir() {
        return Err(format!(
            "OBSIDIAN_AGENT_VAULT_ROOT does not point to a directory: {}",
            canonical.display()
        ));
    }

    Ok(canonical)
}

/// Resolve a path *within* the Obsidian vault, ensuring it does not escape
/// the configured root. The `relative_path` is treated as a vault-internal
/// path (e.g., `projects`, `notes/subdir`).
///
/// The returned path is canonicalized and guaranteed to be a descendant of
/// `vault_root`, or an error is returned.
pub fn resolve_vault_path(vault_root: &Path, relative_path: &str) -> Result<PathBuf, String> {
    if relative_path.trim().is_empty() {
        return Err("vault_path must be a non-empty relative path".to_string());
    }

    let candidate = vault_root.join(relative_path);
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize vault path '{}': {e}", relative_path))?;

    if !canonical.starts_with(vault_root) {
        return Err(format!(
            "vault_path '{}' escapes the configured Obsidian vault root and is not allowed",
            relative_path
        ));
    }

    Ok(canonical)
}
