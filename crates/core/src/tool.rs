use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub parameters: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl ToolInput {
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).and_then(|v| v.as_str())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.parameters.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.parameters.get(key).and_then(|v| v.as_i64())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn info(&self) -> ToolInfo;

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput>;

    fn is_enabled(&self) -> bool {
        true
    }
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let info = tool.info();
        self.tools.insert(info.name.clone(), Arc::new(tool));
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .filter(|t| t.is_enabled())
            .map(|t| t.info())
            .collect()
    }

    pub async fn execute(&self, name: &str, input: ToolInput) -> Result<ToolOutput> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| CoreError::UnknownTool(name.to_string()))?;

        if !tool.is_enabled() {
            return Err(CoreError::ToolError {
                tool: name.to_string(),
                message: "Tool is disabled".to_string(),
            });
        }

        tool.execute(input).await
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    pub fn register_mcp_tool(&mut self, name: &str, description: &str, parameters: Value) {
        let tool = McpPlaceholderTool {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        };
        self.tools.insert(name.to_string(), Arc::new(tool));
    }
}

/// Placeholder for MCP tools not yet fully wired through the MCP client.
/// Returns an error instructing the model that MCP integration is in progress.
pub struct McpPlaceholderTool {
    name: String,
    description: String,
    parameters: Value,
}

#[async_trait]
impl Tool for McpPlaceholderTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: self.name.clone(),
            description: self.description.clone(),
            parameters: self.parameters.clone(),
        }
    }

    async fn execute(&self, _input: ToolInput) -> Result<ToolOutput> {
        Ok(ToolOutput::error(format!(
            "MCP tool '{}' is registered but not yet fully wired. \
             The MCP transport layer (stdio/HTTP/SSE) is under development. \
             Use built-in tools instead.",
            self.name
        )))
    }
}
