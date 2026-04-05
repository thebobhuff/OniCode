use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub servers: HashMap<String, McpServerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpServerEntry {
    Stdio(StdioServer),
    Http(HttpServer),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdioServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServer {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub transport: HttpTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum HttpTransport {
    #[default]
    StreamableHttp,
    Sse,
    WebSocket,
}

impl McpServerConfig {
    pub fn load(path: &std::path::Path) -> Option<Self> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn load_from_dir(dir: &std::path::Path) -> Self {
        let mcp_json = dir.join(".mcp.json");
        Self::load(&mcp_json).unwrap_or_else(|| McpServerConfig {
            servers: HashMap::new(),
        })
    }
}
