use std::{collections::HashMap, path::Path};

use serde::{Deserialize, Serialize};

use crate::error::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    Allow,
    Ask,
    Deny,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Allow => write!(f, "allow"),
            PermissionMode::Ask => write!(f, "ask"),
            PermissionMode::Deny => write!(f, "deny"),
        }
    }
}

impl PermissionMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "allow" | "auto" => PermissionMode::Allow,
            "ask" | "prompt" => PermissionMode::Ask,
            "deny" | "block" => PermissionMode::Deny,
            _ => PermissionMode::Ask,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default)]
    pub provider: String,

    #[serde(default = "default_max_turns")]
    pub max_turns: usize,

    #[serde(default = "default_permission_mode")]
    pub permission_mode: String,

    #[serde(default)]
    pub allowed_tools: Option<Vec<String>>,

    #[serde(default)]
    pub disallowed_tools: Option<Vec<String>>,

    #[serde(default)]
    pub permissions: Option<HashMap<String, PermissionMode>>,

    #[serde(default = "default_true")]
    pub auto_compact: bool,

    #[serde(default)]
    pub always_thinking: bool,

    #[serde(default)]
    pub temperature: Option<f64>,

    #[serde(default)]
    pub max_tokens: Option<u32>,

    #[serde(default)]
    pub system_prompt: Option<String>,

    #[serde(default)]
    pub base_url: Option<String>,

    #[serde(default)]
    pub add_dirs: Vec<String>,

    #[serde(default)]
    pub agent_mode: String,
}

fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}

fn default_max_turns() -> usize {
    100
}

fn default_permission_mode() -> String {
    "auto".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: default_model(),
            provider: "anthropic".to_string(),
            max_turns: default_max_turns(),
            permission_mode: default_permission_mode(),
            allowed_tools: None,
            disallowed_tools: None,
            permissions: None,
            auto_compact: true,
            always_thinking: false,
            temperature: None,
            max_tokens: None,
            system_prompt: None,
            base_url: None,
            add_dirs: Vec::new(),
            agent_mode: "build".to_string(),
        }
    }
}

impl Settings {
    pub async fn load(workspace_root: &Path) -> Self {
        let config_dir = workspace_root.join(".onicode");
        let global_dir = dirs::config_dir()
            .map(|d| d.join("onicode"))
            .unwrap_or_else(|| Path::new(".onicode").to_path_buf());

        let paths = [
            config_dir.join("settings.local.json"),
            config_dir.join("settings.json"),
            global_dir.join("settings.json"),
        ];

        let mut settings = Self::default();

        for path in paths.iter().rev() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(loaded) = serde_json::from_str::<Self>(&content) {
                        settings = loaded;
                    }
                }
            }
        }

        settings.override_from_env();
        settings
    }

    fn override_from_env(&mut self) {
        if let Ok(model) = std::env::var("ONICODE_MODEL") {
            self.model = model;
        }
        if let Ok(provider) = std::env::var("ONICODE_PROVIDER") {
            self.provider = provider;
        }
        if let Ok(max_turns) = std::env::var("ONICODE_MAX_TURNS") {
            if let Ok(n) = max_turns.parse() {
                self.max_turns = n;
            }
        }
    }

    pub async fn save(&self, workspace_root: &Path) -> Result<()> {
        let config_dir = workspace_root.join(".onicode");
        std::fs::create_dir_all(&config_dir).map_err(|e| ConfigError::Io(e))?;

        let settings_path = config_dir.join("settings.json");
        let content = serde_json::to_string_pretty(self).map_err(|e| ConfigError::Serde(e))?;
        std::fs::write(&settings_path, content).map_err(|e| ConfigError::Io(e))?;

        Ok(())
    }
}
