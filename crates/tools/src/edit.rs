use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct EditTool {
    working_directory: String,
}

impl EditTool {
    pub fn new(working_directory: String) -> Self {
        Self { working_directory }
    }
}

#[async_trait]
impl Tool for EditTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Edit".to_string(),
            description: "Replace a specific string in a file. The old_string must match exactly (including whitespace). Use for surgical edits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "Exact text to find and replace (must match exactly)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Text to replace old_string with"
                    }
                },
                "required": ["file_path", "old_string", "new_string"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let file_path =
            input
                .get_str("file_path")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Edit".to_string(),
                    message: "Missing 'file_path' parameter".to_string(),
                })?;

        let old_string =
            input
                .get_str("old_string")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Edit".to_string(),
                    message: "Missing 'old_string' parameter".to_string(),
                })?;

        let new_string =
            input
                .get_str("new_string")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Edit".to_string(),
                    message: "Missing 'new_string' parameter".to_string(),
                })?;

        let full_path = Path::new(&self.working_directory).join(file_path);

        debug!(
            file = %full_path.display(),
            "Applying edit"
        );

        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ToolOutput::error(format!("Failed to read file: {e}")));
            }
        };

        let occurrences = content.matches(old_string).count();

        if occurrences == 0 {
            return Ok(ToolOutput::error(format!(
                "String not found in {}",
                full_path.display()
            )));
        }

        if occurrences > 1 {
            return Ok(ToolOutput::error(format!(
                "String found {occurrences} times. Make old_string more specific to target a single occurrence."
            )));
        }

        let new_content = content.replace(old_string, new_string);

        if let Err(e) = std::fs::write(&full_path, &new_content) {
            return Ok(ToolOutput::error(format!("Failed to write file: {e}")));
        }

        let old_lines = old_string.lines().count();
        let new_lines = new_string.lines().count();
        let line_delta = new_lines as i64 - old_lines as i64;

        let sign = if line_delta > 0 { "+" } else { "" };
        Ok(ToolOutput::success(format!(
            "Edited {} ({} line{sign})",
            full_path.display(),
            line_delta
        )))
    }
}
