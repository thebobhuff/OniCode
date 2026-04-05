use std::path::Path;

use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct GrepTool {
    working_directory: String,
    max_results: usize,
}

impl GrepTool {
    pub fn new(working_directory: String, max_results: usize) -> Self {
        Self {
            working_directory,
            max_results,
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Grep".to_string(),
            description: "Search file contents using regular expressions. Returns matching lines with file paths and line numbers.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search in (default: current directory)"
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "description": "Whether the search is case sensitive (default: false)"
                    },
                    "include": {
                        "type": "string",
                        "description": "Glob pattern for files to include (e.g., '*.rs')"
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
                    tool: "Grep".to_string(),
                    message: "Missing 'pattern' parameter".to_string(),
                })?;

        let search_path = input.get_str("path").unwrap_or(&self.working_directory);

        let case_sensitive = input.get_bool("case_sensitive").unwrap_or(false);
        let include = input.get_str("include");

        let full_path = Path::new(&self.working_directory).join(search_path);

        debug!(
            pattern,
            path = %full_path.display(),
            case_sensitive,
            "Searching with grep"
        );

        let regex = if case_sensitive {
            regex::Regex::new(pattern)
        } else {
            regex::RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
        }
        .map_err(|e| onicode_core::CoreError::ToolError {
            tool: "Grep".to_string(),
            message: format!("Invalid regex: {e}"),
        })?;

        let walker = ignore::WalkBuilder::new(&self.working_directory)
            .hidden(false)
            .git_ignore(true)
            .build();

        let mut results = Vec::new();

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    if !path.is_file() {
                        continue;
                    }

                    if let Some(include_pat) = include {
                        let glob = match glob::Pattern::new(include_pat) {
                            Ok(g) => g,
                            Err(_) => continue,
                        };
                        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !glob.matches(filename) {
                            continue;
                        }
                    }

                    let content = match std::fs::read_to_string(path) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };

                    for (line_num, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            let relative =
                                path.strip_prefix(&self.working_directory).unwrap_or(path);
                            results.push(format!(
                                "{}:{}:{}",
                                relative.display(),
                                line_num + 1,
                                line
                            ));

                            if results.len() >= self.max_results {
                                break;
                            }
                        }
                    }

                    if results.len() >= self.max_results {
                        break;
                    }
                }
                Err(_) => continue,
            }
        }

        if results.is_empty() {
            Ok(ToolOutput::success(format!(
                "No matches found for pattern: {pattern}"
            )))
        } else {
            Ok(ToolOutput::success(format!(
                "Found {} matches for '{pattern}':\n{}",
                results.len(),
                results.join("\n")
            )))
        }
    }
}
