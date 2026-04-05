use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub commands: Vec<String>,
}

impl Skill {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e))?;
        let (frontmatter, body) = Self::parse_frontmatter(&content)?;

        let mut skill: Skill =
            serde_yaml::from_str(&frontmatter).map_err(|e| ConfigError::Parse(e.to_string()))?;

        if skill.system_prompt.is_none() {
            skill.system_prompt = Some(body.trim().to_string());
        }

        if skill.commands.is_empty() {
            skill.commands = body
                .lines()
                .filter(|l| l.starts_with(|c: char| c.is_ascii_digit()))
                .map(|l| l.trim().to_string())
                .collect();
        }

        Ok(skill)
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
