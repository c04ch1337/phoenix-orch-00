use regex::Regex;
use shared_types::{
    AgentCircuitBreakerConfig, AgentExecutionConfig, AgentRetryConfig, AgentsConfig, AppConfig,
};
use std::env;
use std::fs;
use toml;

/// Load a single TOML config file and perform simple environment interpolation
/// for occurrences of `{{VAR}}` or `${VAR}`.
pub fn load_single_config(path: &str) -> Result<AppConfig, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

    // Regex to find {{VAR_NAME}} or ${VAR_NAME}
    let re = Regex::new(r"(\{\{|\$\{)([a-zA-Z0-9_]+)(\}\}|\})")
        .map_err(|e| format!("Failed to create regex: {}", e))?;

    let processed_content = re.replace_all(&content, |caps: &regex::Captures| {
        let var_name = &caps[2];
        env::var(var_name).unwrap_or_else(|_| format!("{{{{{{{}}}}}}}", var_name))
    });

    let config: AppConfig = toml::from_str(&processed_content)
        .map_err(|e| format!("Failed to parse config file: {}", e))?;
    Ok(config)
}

/// Backwards-compatible wrapper for older call sites. This now just delegates
/// to `load_single_config`.
pub fn load_config(path: &str) -> Result<AppConfig, String> {
    load_single_config(path)
}

/// Merge two AppConfig instances, treating `overlay` as an environment-specific
/// override on top of `base`.
///
/// Rules:
/// - `llm.default_provider` in overlay replaces base when present.
/// - For each provider under `llm`, non-None overlay entries replace base.
/// - `agents`:
///   - If overlay.agents is Some, it shadows/overrides base.agents with a
///     field-wise merge for each known agent (default, git_agent,
///     obsidian_agent, llm_router_agent).
pub fn merge_app_config(base: AppConfig, overlay: AppConfig) -> AppConfig {
    // Merge LLM config first.
    let mut merged_llm = base.llm;

    // default_provider
    if !overlay.llm.default_provider.is_empty()
        && overlay.llm.default_provider != merged_llm.default_provider
    {
        merged_llm.default_provider = overlay.llm.default_provider;
    }

    macro_rules! merge_provider {
        ($field:ident) => {
            if let Some(ov) = overlay.llm.$field {
                merged_llm.$field = Some(ov);
            }
        };
    }

    merge_provider!(openrouter);
    merge_provider!(gemini);
    merge_provider!(grok);
    merge_provider!(openai);
    merge_provider!(anthropic);
    merge_provider!(ollama);
    merge_provider!(lmstudio);

    // Merge agents config.
    let merged_agents: Option<AgentsConfig> = match (base.agents, overlay.agents) {
        (Some(base_agents), Some(overlay_agents)) => {
            Some(merge_agents_config(base_agents, overlay_agents))
        }
        (None, Some(overlay_agents)) => Some(overlay_agents),
        (Some(base_agents), None) => Some(base_agents),
        (None, None) => None,
    };

    AppConfig {
        llm: merged_llm,
        agents: merged_agents,
    }
}

fn merge_agents_config(base: AgentsConfig, overlay: AgentsConfig) -> AgentsConfig {
    AgentsConfig {
        default: merge_agent_execution_config(base.default, overlay.default),
        git_agent: merge_agent_opt(base.git_agent, overlay.git_agent),
        obsidian_agent: merge_agent_opt(base.obsidian_agent, overlay.obsidian_agent),
        llm_router_agent: merge_agent_opt(base.llm_router_agent, overlay.llm_router_agent),
    }
}

fn merge_agent_opt(
    base: Option<AgentExecutionConfig>,
    overlay: Option<AgentExecutionConfig>,
) -> Option<AgentExecutionConfig> {
    match (base, overlay) {
        (Some(b), Some(o)) => Some(merge_agent_execution_config(b, o)),
        (None, Some(o)) => Some(o),
        (Some(b), None) => Some(b),
        (None, None) => None,
    }
}

fn merge_agent_execution_config(
    base: AgentExecutionConfig,
    overlay: AgentExecutionConfig,
) -> AgentExecutionConfig {
    AgentExecutionConfig {
        timeout_secs: if overlay.timeout_secs != 0 {
            overlay.timeout_secs
        } else {
            base.timeout_secs
        },
        retry: AgentRetryConfig {
            max_attempts: if overlay.retry.max_attempts != 0 {
                overlay.retry.max_attempts
            } else {
                base.retry.max_attempts
            },
            initial_backoff_ms: if overlay.retry.initial_backoff_ms != 0 {
                overlay.retry.initial_backoff_ms
            } else {
                base.retry.initial_backoff_ms
            },
            max_backoff_ms: if overlay.retry.max_backoff_ms != 0 {
                overlay.retry.max_backoff_ms
            } else {
                base.retry.max_backoff_ms
            },
        },
        circuit_breaker: AgentCircuitBreakerConfig {
            failure_threshold: if overlay.circuit_breaker.failure_threshold != 0 {
                overlay.circuit_breaker.failure_threshold
            } else {
                base.circuit_breaker.failure_threshold
            },
            cooldown_ms: if overlay.circuit_breaker.cooldown_ms != 0 {
                overlay.circuit_breaker.cooldown_ms
            } else {
                base.circuit_breaker.cooldown_ms
            },
        },
    }
}

/// Load configuration using an environment profile, overlaying
/// `data/config.<env>.toml` on top of the base `data/config.toml`.
///
/// Example:
/// - base_path: "data/config.toml"
/// - env: "dev" | "staging" | "prod"
pub fn load_app_config_with_env(base_path: &str, env_name: &str) -> Result<AppConfig, String> {
    let base = load_single_config(base_path)?;

    let env = env_name.to_lowercase();
    let overlay_path = if env == "dev" {
        // For dev, it is valid to have only the base config.
        format!("data/config.dev.toml")
    } else {
        format!("data/config.{}.toml", env)
    };

    // Try to load the overlay; if it does not exist, just return base.
    let overlay = match load_single_config(&overlay_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            // If the file is missing, treat that as "no overlay" and return base.
            if e.contains("Failed to read config file") {
                return Ok(base);
            }
            return Err(e);
        }
    };

    Ok(merge_app_config(base, overlay))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_minimal_app_config(default_provider: &str, timeout_secs: u64) -> AppConfig {
        AppConfig {
            llm: shared_types::LLMConfig {
                default_provider: default_provider.to_string(),
                openrouter: None,
                gemini: None,
                grok: None,
                openai: None,
                anthropic: None,
                ollama: None,
                lmstudio: None,
            },
            agents: Some(AgentsConfig {
                default: AgentExecutionConfig {
                    timeout_secs,
                    retry: AgentRetryConfig {
                        max_attempts: 3,
                        initial_backoff_ms: 100,
                        max_backoff_ms: 1_000,
                    },
                    circuit_breaker: AgentCircuitBreakerConfig {
                        failure_threshold: 3,
                        cooldown_ms: 60_000,
                    },
                },
                git_agent: None,
                obsidian_agent: None,
                llm_router_agent: None,
            }),
        }
    }

    #[test]
    fn load_single_config_substitutes_env_vars_brace_syntax() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "{}",
            r#"[llm]
default_provider = "openrouter"

[llm.openrouter]
api_key = "{{OPENROUTER_API_KEY}}"
model_name = "test-model"
"#
        )
        .expect("write config");

        env::set_var("OPENROUTER_API_KEY", "test-key-123");

        let path_str = file.path().to_str().unwrap().to_string();
        let cfg = load_single_config(&path_str).expect("config should load");

        assert_eq!(cfg.llm.default_provider, "openrouter");
        let openrouter = cfg.llm.openrouter.expect("openrouter config present");
        assert_eq!(openrouter.api_key.as_deref(), Some("test-key-123"));
        assert_eq!(openrouter.model_name, "test-model");
    }

    #[test]
    fn load_single_config_substitutes_env_vars_dollar_syntax() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(
            file,
            "{}",
            r#"[llm]
default_provider = "anthropic"

[llm.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model_name = "claude-test"
"#
        )
        .expect("write config");

        env::set_var("ANTHROPIC_API_KEY", "anthropic-key-xyz");

        let path_str = file.path().to_str().unwrap().to_string();
        let cfg = load_single_config(&path_str).expect("config should load");

        assert_eq!(cfg.llm.default_provider, "anthropic");
        let anthropic = cfg.llm.anthropic.expect("anthropic config present");
        assert_eq!(anthropic.api_key.as_deref(), Some("anthropic-key-xyz"));
        assert_eq!(anthropic.model_name, "claude-test");
    }

    #[test]
    fn merge_app_config_overlay_wins_for_llm_and_agents() {
        let base = make_minimal_app_config("openrouter", 30);
        let mut overlay = make_minimal_app_config("openai", 0);

        // In the overlay, set a non-zero timeout so it should override base.
        if let Some(ref mut agents) = overlay.agents {
            agents.default.timeout_secs = 45;
        }

        let merged = merge_app_config(base, overlay);

        // default_provider from overlay should win.
        assert_eq!(merged.llm.default_provider, "openai");

        // Agents config should prefer non-zero values from overlay.
        let agents = merged.agents.expect("agents config present");
        assert_eq!(agents.default.timeout_secs, 45);
    }
}
