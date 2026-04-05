use serde_json::Value;

#[derive(Debug, Clone)]
pub struct McpTool {
    pub server_name: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpTool {
    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.server_name, self.name)
    }
}
