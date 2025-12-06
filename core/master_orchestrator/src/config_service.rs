use shared_types::AppConfig;
use std::fs;
use toml;
use regex::Regex;
use std::env;

pub fn load_config(path: &str) -> Result<AppConfig, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;
    
    // Regex to find {{VAR_NAME}}
    let re = Regex::new(r"\{\{([a-zA-Z0-9_]+)\}\}").map_err(|e| format!("Failed to create regex: {}", e))?;
    
    let processed_content = re.replace_all(&content, |caps: &regex::Captures| {
        let var_name = &caps[1];
        env::var(var_name).unwrap_or_else(|_| format!("{{{{{}}}}}", var_name))
    });

    let config: AppConfig = toml::from_str(&processed_content).map_err(|e| format!("Failed to parse config file: {}", e))?;
    Ok(config)
}
