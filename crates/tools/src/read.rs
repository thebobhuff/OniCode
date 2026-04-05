use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct ReadTool {
    working_directory: String,
    max_lines: usize,
}

impl ReadTool {
    pub fn new(working_directory: String, max_lines: usize) -> Self {
        Self {
            working_directory,
            max_lines,
        }
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Read".to_string(),
            description: "Read the contents of a file with line numbers. Use offset and limit to read specific sections.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Starting line number (1-indexed, default: 1)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let file_path =
            input
                .get_str("file_path")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Read".to_string(),
                    message: "Missing 'file_path' parameter".to_string(),
                })?;

        let offset = input.get_i64("offset").unwrap_or(1) as usize;
        let limit = input
            .get_i64("limit")
            .map(|l| l as usize)
            .unwrap_or(self.max_lines);

        let full_path = Path::new(&self.working_directory).join(file_path);

        debug!(file = %full_path.display(), offset, limit, "Reading file");

        if !full_path.exists() {
            return Ok(ToolOutput::error(format!(
                "File not found: {}",
                full_path.display()
            )));
        }

        if !full_path.is_file() {
            return Ok(ToolOutput::error(format!(
                "Not a file: {}",
                full_path.display()
            )));
        }

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolOutput::error(format!("Failed to read file: {e}")));
            }
        };

        let total_lines = content.lines().count();
        let start = (offset - 1).min(total_lines);
        let end = (start + limit).min(total_lines);

        let lines: Vec<String> = content
            .lines()
            .skip(start)
            .take(limit)
            .enumerate()
            .map(|(i, line)| format!("{:>6}: {line}", start + i + 1))
            .collect();

        let output = if lines.is_empty() {
            format!("(empty file, {total_lines} lines total)")
        } else {
            let header = format!(
                "📄 {} (lines {}-{} of {total_lines})\n{}",
                full_path.display(),
                start + 1,
                end,
                "─".repeat(60)
            );
            format!("{header}\n{}", lines.join("\n"))
        };

        Ok(ToolOutput::success(output))
    }
}
