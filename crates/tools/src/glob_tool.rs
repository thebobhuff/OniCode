use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct GlobTool {
    working_directory: String,
}

impl GlobTool {
    pub fn new(working_directory: String) -> Self {
        Self { working_directory }
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Glob".to_string(),
            description: "Find files matching a glob pattern. Supports ** for recursive matching."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern (e.g., '**/*.rs', 'src/**/*.ts')"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let pattern =
            input
                .get_str("pattern")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Glob".to_string(),
                    message: "Missing 'pattern' parameter".to_string(),
                })?;

        let full_pattern = Path::new(&self.working_directory).join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        debug!(pattern = %pattern_str, "Searching with glob");

        let walker = ignore::WalkBuilder::new(&self.working_directory)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut matches = Vec::new();
        let pattern_glob =
            glob::Pattern::new(pattern).map_err(|e| onicode_core::CoreError::ToolError {
                tool: "Glob".to_string(),
                message: format!("Invalid glob pattern: {e}"),
            })?;

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    let relative = path.strip_prefix(&self.working_directory).unwrap_or(path);
                    let rel_str = relative.to_string_lossy();

                    if pattern_glob.matches(&rel_str) {
                        matches.push(rel_str.to_string());
                    }
                }
                Err(_) => continue,
            }
        }

        matches.sort();

        if matches.is_empty() {
            Ok(ToolOutput::success(format!(
                "No files matched pattern: {pattern}"
            )))
        } else {
            Ok(ToolOutput::success(format!(
                "Found {} files matching '{pattern}':\n{}",
                matches.len(),
                matches.join("\n")
            )))
        }
    }
}
