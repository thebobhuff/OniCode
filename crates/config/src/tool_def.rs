use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub script: Option<String>,
    pub parameters: Vec<ToolParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

impl ToolDef {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Io(e))?;
        let (frontmatter, body) = Self::parse_frontmatter(&content)?;

        let mut tool: ToolDef =
            serde_yaml::from_str(&frontmatter).map_err(|e| ConfigError::Parse(e.to_string()))?;

        if tool.script.is_none() {
            let dir = path.parent().unwrap();
            let scripts_dir = dir
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("scripts"));

            if let Some(scripts_dir) = scripts_dir {
                if scripts_dir.exists() {
                    let tool_name_lower = tool.name.to_lowercase();
                    for ext in &["sh", "ps1", "bat", "py"] {
                        let script_path = scripts_dir.join(format!("{}.{}", tool_name_lower, ext));
                        if script_path.exists() {
                            tool.script = Some(script_path.to_string_lossy().to_string());
                            break;
                        }
                    }
                }
            }
        }

        if tool.parameters.is_empty() {
            tool.parameters = Self::parse_parameters_from_body(&body);
        }

        Ok(tool)
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

    fn parse_parameters_from_body(body: &str) -> Vec<ToolParameter> {
        let mut params = Vec::new();

        for line in body.lines() {
            let line = line.trim();
            if line.starts_with('-') || line.starts_with('*') {
                let content = line.trim_start_matches(|c| c == '-' || c == '*').trim();
                if let Some((name, desc)) = content
                    .splitn(2, ':')
                    .next()
                    .zip(content.splitn(2, ':').nth(1))
                {
                    params.push(ToolParameter {
                        name: name.trim().to_string(),
                        description: desc.trim().to_string(),
                        required: false,
                        default: None,
                    });
                }
            }
        }

        params
    }
}
