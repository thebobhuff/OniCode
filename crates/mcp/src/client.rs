use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::{McpServerConfig, McpServerEntry, StdioServer, HttpServer};
use crate::error::{McpError, Result};
use crate::tool::McpTool;

pub struct McpClient {
    servers: Arc<RwLock<HashMap<String, ServerConnection>>>,
}

struct ServerConnection {
    name: String,
    tools: Vec<McpTool>,
    #[allow(dead_code)]
    child: Option<tokio::process::Child>,
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn connect_all(&self, config: &McpServerConfig) -> Result<()> {
        for (name, entry) in &config.servers {
            if let Err(e) = self.connect(name, entry).await {
                warn!(server = name, error = %e, "Failed to connect to MCP server");
            }
        }
        Ok(())
    }

    async fn connect(&self, name: &str, entry: &McpServerEntry) -> Result<()> {
        info!(server = name, "Connecting to MCP server");

        let (tools, child) = match entry {
            McpServerEntry::Stdio(stdio) => self.connect_stdio(name, stdio).await?,
            McpServerEntry::Http(http) => self.connect_http(name, http).await?,
        };

        let connection = ServerConnection {
            name: name.to_string(),
            tools,
            child,
        };

        self.servers.write().await.insert(name.to_string(), connection);

        info!(server = name, "Connected to MCP server");
        Ok(())
    }

    async fn connect_stdio(
        &self,
        _name: &str,
        config: &StdioServer,
    ) -> Result<(Vec<McpTool>, Option<tokio::process::Child>)> {
        debug!(
            command = %config.command,
            args = ?config.args,
            "Connecting to stdio MCP server"
        );

        // Spawn the MCP server process
        let mut child = tokio::process::Command::new(&config.command)
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&config.env)
            .spawn()
            .map_err(|e| McpError::ConnectionFailed(format!("Failed to spawn: {e}")))?;

        // TODO: Wire up JSON-RPC over stdin/stdout with rmcp
        // For now, return empty tools list. The full integration requires:
        // 1. Creating an IoTransport from stdin/stdout pipes
        // 2. Calling serve_client with the transport
        // 3. Sending initialize + tools/list requests

        // Kill the child since we can't use it yet
        let _ = child.kill().await;

        Ok((Vec::new(), None))
    }

    async fn connect_http(
        &self,
        _name: &str,
        config: &HttpServer,
    ) -> Result<(Vec<McpTool>, Option<tokio::process::Child>)> {
        debug!(
            url = %config.url,
            transport = ?config.transport,
            "Connecting to HTTP MCP server"
        );

        // TODO: Wire up HTTP/SSE transport with rmcp
        warn!("HTTP MCP transport not yet implemented");
        Ok((Vec::new(), None))
    }

    pub async fn list_all_tools(&self) -> Vec<McpTool> {
        let servers = self.servers.read().await;
        servers
            .values()
            .flat_map(|s| s.tools.clone())
            .collect()
    }

    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        _args: Value,
    ) -> Result<String> {
        let servers = self.servers.read().await;
        let _server = servers.get(server_name).ok_or_else(|| {
            McpError::ToolCallFailed {
                tool: tool_name.to_string(),
                message: format!("Server '{server_name}' not connected"),
            }
        })?;

        // TODO: Implement actual tool call via rmcp
        Err(McpError::ToolCallFailed {
            tool: tool_name.to_string(),
            message: "MCP tool calls not yet fully implemented".into(),
        })
    }

    pub async fn disconnect(&self, server_name: &str) -> Result<()> {
        let mut servers = self.servers.write().await;
        if let Some(conn) = servers.remove(server_name) {
            if let Some(mut child) = conn.child {
                let _ = child.kill().await;
            }
            info!(server = server_name, "Disconnected from MCP server");
        }
        Ok(())
    }

    pub async fn disconnect_all(&self) {
        let servers = self.servers.read().await;
        let names: Vec<String> = servers.keys().cloned().collect();
        drop(servers);

        for name in names {
            let _ = self.disconnect(&name).await;
        }
    }

    pub fn server_count(&self) -> usize {
        self.servers.try_read().map(|s| s.len()).unwrap_or(0)
    }
}
