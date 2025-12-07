use rusqlite::Connection;
use serde_json;
use shared_types::Tool;

pub fn initialize_database(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tool_registry (
            name TEXT PRIMARY KEY,
            version TEXT NOT NULL,
            description TEXT NOT NULL,
            executable_path TEXT NOT NULL,
            actions_schema TEXT NOT NULL,
            tags TEXT,
            category TEXT,
            enabled BOOLEAN NOT NULL
        )",
        [],
    )?;
    Ok(())
}

pub fn register_tool(conn: &Connection, tool: &Tool) -> rusqlite::Result<()> {
    let actions_schema_json =
        serde_json::to_string(&tool.actions_schema).unwrap_or_else(|_| "{}".to_string());

    conn.execute(
        "INSERT OR REPLACE INTO tool_registry (name, version, description, executable_path, actions_schema, tags, category, enabled) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            tool.name,
            tool.version,
            tool.description,
            tool.executable_path,
            actions_schema_json,
            tool.tags,
            tool.category,
            tool.enabled
        ],
    )?;
    Ok(())
}

pub fn load_tools(conn: &rusqlite::Connection) -> Result<Vec<Tool>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT name, version, description, executable_path, actions_schema, tags, category, enabled FROM tool_registry")?;
    let tool_iter = stmt.query_map([], |row| {
        let actions_schema_str: String = row.get(4)?;
        let actions_schema = serde_json::from_str(&actions_schema_str).unwrap_or_default();

        Ok(Tool {
            name: row.get(0)?,
            version: row.get(1)?,
            description: row.get(2)?,
            executable_path: row.get(3)?,
            actions_schema,
            tags: row.get(5)?,
            category: row.get(6)?,
            enabled: row.get(7)?,
        })
    })?;

    let mut tools = Vec::new();
    for tool in tool_iter {
        tools.push(tool?);
    }
    Ok(tools)
}
