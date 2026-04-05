use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

fn default_permission_mode() -> String {
    "auto".to_string()
}

impl AgentConfig {
    pub fn default_build() -> Self {
        Self {
            name: "build".to_string(),
            description: "Default, full-access agent for development work".to_string(),
            model: None,
            tools: None,
            permission_mode: "auto".to_string(),
            system_prompt: None,
        }
    }

    pub fn default_plan() -> Self {
        Self {
            name: "plan".to_string(),
            description: "Read-only agent for analysis and code exploration".to_string(),
            model: None,
            tools: Some(vec![
                "Read".into(),
                "Glob".into(),
                "Grep".into(),
                "LS".into(),
                "Bash".into(),
            ]),
            permission_mode: "plan".to_string(),
            system_prompt: Some("You are a read-only analysis agent. You should not make edits. Ask permission before running bash commands.".into()),
        }
    }

    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e))?;

        let (frontmatter, body) = Self::parse_frontmatter(&content)?;

        let mut agent: AgentConfig =
            serde_yaml::from_str(&frontmatter).map_err(|e| ConfigError::Parse(e.to_string()))?;

        if agent.system_prompt.is_none() {
            agent.system_prompt = Some(body.trim().to_string());
        }

        Ok(agent)
    }

    fn parse_frontmatter(content: &str) -> Result<(String, String)> {
        let content = content.trim_start_matches('\u{feff}');

        if !content.starts_with("---") {
            return Err(ConfigError::Parse("Missing YAML frontmatter".to_string()));
        }

        let rest = &content[3..];
        if let Some(end) = rest.find("\n---") {
            let frontmatter = rest[..end].trim().to_string();
            let body = rest[end + 4..].trim().to_string();
            Ok((frontmatter, body))
        } else {
            Err(ConfigError::Parse("Unclosed frontmatter".to_string()))
        }
    }
}
