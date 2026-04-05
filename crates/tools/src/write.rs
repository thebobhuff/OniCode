use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct WriteTool {
    working_directory: String,
}

impl WriteTool {
    pub fn new(working_directory: String) -> Self {
        Self { working_directory }
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Write".to_string(),
            description: "Create or overwrite a file with the given content. Creates parent directories if they don't exist.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let file_path =
            input
                .get_str("file_path")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Write".to_string(),
                    message: "Missing 'file_path' parameter".to_string(),
                })?;

        let content =
            input
                .get_str("content")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Write".to_string(),
                    message: "Missing 'content' parameter".to_string(),
                })?;

        let full_path = Path::new(&self.working_directory).join(file_path);

        debug!(file = %full_path.display(), bytes = content.len(), "Writing file");

        if let Some(parent) = full_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return Ok(ToolOutput::error(format!(
                    "Failed to create directory: {e}"
                )));
            }
        }

        if let Err(e) = std::fs::write(&full_path, content) {
            return Ok(ToolOutput::error(format!("Failed to write file: {e}")));
        }

        let lines = content.lines().count();
        let bytes = content.len();

        Ok(ToolOutput::success(format!(
            "Wrote {bytes} bytes ({lines} lines) to {}",
            full_path.display()
        )))
    }
}
