use crate::tool_registry_service;
use rusqlite::Connection;
use shared_types::Tool;
use std::collections::HashMap;

/// Service responsible for loading and invoking registered tools.
pub struct ToolService {
    pub tools: HashMap<String, Tool>,
}

impl ToolService {
    /// Create a new ToolService by loading tools from the registry database.
    pub fn new(conn: &Connection) -> Result<Self, String> {
        let tools = tool_registry_service::load_tools(conn).map_err(|e| e.to_string())?;

        let mut tool_map = HashMap::new();
        for tool in tools {
            tool_map.insert(tool.name.clone(), tool);
        }

        Ok(ToolService { tools: tool_map })
    }

    /// Execute a registered tool by spawning its executable with the given params.
    pub async fn execute_tool(&self, tool_name: &str, params: &[&str]) -> Result<String, String> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| "Tool not found".to_string())?;

        let output = tokio::process::Command::new(&tool.executable_path)
            .args(params)
            .output()
            .await
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
