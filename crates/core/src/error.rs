use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("LLM request failed: {0}")]
    LlmError(String),

    #[error("Tool execution failed: {tool}: {message}")]
    ToolError { tool: String, message: String },

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Agent loop terminated: {0}")]
    AgentLoopTerminated(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
