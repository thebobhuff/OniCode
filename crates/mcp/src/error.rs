use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Tool call failed: {tool}: {message}")]
    ToolCallFailed { tool: String, message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("MCP SDK error: {0}")]
    Sdk(String),
}

pub type Result<T> = std::result::Result<T, McpError>;
