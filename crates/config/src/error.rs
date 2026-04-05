use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    Parse(String),

    #[error("Invalid config value: {0}")]
    InvalidValue(String),

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Failed to serialize config: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;
