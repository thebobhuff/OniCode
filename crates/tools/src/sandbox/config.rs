use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub network_access: bool,

    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,

    #[serde(default)]
    pub denied_paths: Vec<PathBuf>,

    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,

    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
}

fn default_true() -> bool {
    true
}

fn default_max_memory_mb() -> u64 {
    512
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_max_output_bytes() -> usize {
    1024 * 1024
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            network_access: false,
            allowed_paths: Vec::new(),
            denied_paths: Vec::new(),
            max_memory_mb: 512,
            timeout_secs: 30,
            max_output_bytes: 1024 * 1024,
        }
    }
}

impl SandboxConfig {
    pub fn safe_default() -> Self {
        Self {
            enabled: true,
            network_access: false,
            allowed_paths: vec![PathBuf::from(".")],
            denied_paths: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/root"),
                PathBuf::from("/home"),
                PathBuf::from("C:\\Windows"),
                PathBuf::from("C:\\Users"),
                PathBuf::from("/Users"),
            ],
            max_memory_mb: 256,
            timeout_secs: 15,
            max_output_bytes: 512 * 1024,
        }
    }

    pub fn permissive() -> Self {
        Self {
            enabled: false,
            network_access: true,
            ..Self::default()
        }
    }
}
