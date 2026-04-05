use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct LsTool {
    working_directory: String,
}

impl LsTool {
    pub fn new(working_directory: String) -> Self {
        Self { working_directory }
    }
}

#[async_trait]
impl Tool for LsTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "LS".to_string(),
            description:
                "List directory contents with file types and sizes. Shows hidden files by default."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Directory to list (default: current directory)"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "Include hidden files (default: true)"
                    }
                },
                "required": []
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let path = input.get_str("path").unwrap_or(".");

        let show_all = input.get_bool("all").unwrap_or(true);

        let full_path = Path::new(&self.working_directory).join(path);

        debug!(path = %full_path.display(), "Listing directory");

        if !full_path.exists() {
            return Ok(ToolOutput::error(format!(
                "Path not found: {}",
                full_path.display()
            )));
        }

        if !full_path.is_dir() {
            return Ok(ToolOutput::error(format!(
                "Not a directory: {}",
                full_path.display()
            )));
        }

        let entries = match std::fs::read_dir(&full_path) {
            Ok(e) => e,
            Err(e) => {
                return Ok(ToolOutput::error(format!("Failed to read directory: {e}")));
            }
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries {
            match entry {
                Ok(entry) => {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();

                    if !show_all && name_str.starts_with('.') {
                        continue;
                    }

                    let metadata = match entry.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    let size = metadata.len();
                    let is_dir = metadata.is_dir();
                    let is_symlink = metadata.file_type().is_symlink();

                    let size_str = format_size(size);
                    let type_indicator = if is_dir {
                        "📁"
                    } else if is_symlink {
                        "🔗"
                    } else {
                        "📄"
                    };

                    let line = format!("  {type_indicator} {size_str:>8}  {name_str}");

                    if is_dir {
                        dirs.push(line);
                    } else {
                        files.push(line);
                    }
                }
                Err(_) => continue,
            }
        }

        dirs.sort();
        files.sort();

        let mut output = format!("📂 {}\n{}\n", full_path.display(), "─".repeat(40));
        output.push_str(&dirs.join("\n"));
        if !dirs.is_empty() && !files.is_empty() {
            output.push('\n');
        }
        output.push_str(&files.join("\n"));

        let total = dirs.len() + files.len();
        output.push_str(&format!("\n\n{total} entries"));

        Ok(ToolOutput::success(output))
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
