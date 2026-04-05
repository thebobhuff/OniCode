use std::process::Command;

use async_trait::async_trait;
use onicode_core::tool::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;

pub struct GhPrCreateTool;

#[async_trait]
impl Tool for GhPrCreateTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "gh_pr_create".to_string(),
            description: "Create a GitHub Pull Request using the gh CLI".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["title", "body"],
                "properties": {
                    "title": { "type": "string", "description": "PR title" },
                    "body": { "type": "string", "description": "PR description" },
                    "base": { "type": "string", "description": "Target branch (default: main)" },
                    "head": { "type": "string", "description": "Source branch (default: current)" },
                }
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput, onicode_core::CoreError> {
        let title = input.get_str("title").unwrap_or("Update");
        let body = input.get_str("body").unwrap_or("");
        let base = input.get_str("base").unwrap_or("main");
        let head = input.get_str("head").unwrap_or("");

        let mut cmd = Command::new("gh");
        cmd.args([
            "pr", "create", "--title", title, "--body", body, "--base", base,
        ]);

        if !head.is_empty() {
            cmd.args(["--head", head]);
        }

        let output = cmd
            .output()
            .map_err(|e| onicode_core::CoreError::ToolError {
                tool: "gh_pr_create".to_string(),
                message: format!("Failed to execute gh CLI: {}", e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            let pr_url = stdout.trim();
            let pr_number = pr_url.rsplit('/').next().unwrap_or("").trim().to_string();

            Ok(ToolOutput::success(format!(
                "PR created: {}\nURL: {}",
                pr_number, pr_url
            )))
        } else {
            Err(onicode_core::CoreError::ToolError {
                tool: "gh_pr_create".to_string(),
                message: format!("gh pr create failed: {}", stderr),
            })
        }
    }
}

pub struct GhPrMergeTool;

#[async_trait]
impl Tool for GhPrMergeTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "gh_pr_merge".to_string(),
            description: "Merge a GitHub Pull Request using the gh CLI".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["pr_number"],
                "properties": {
                    "pr_number": { "type": "string", "description": "PR number to merge" },
                    "method": { "type": "string", "description": "merge, squash, or rebase (default: squash)" },
                    "delete_branch": { "type": "boolean", "description": "Delete branch after merge (default: true)" },
                }
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput, onicode_core::CoreError> {
        let pr_number =
            input
                .get_str("pr_number")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "gh_pr_merge".to_string(),
                    message: "pr_number is required".to_string(),
                })?;

        let method = input.get_str("method").unwrap_or("squash");
        let delete_branch = input.get_bool("delete_branch").unwrap_or(true);

        let mut cmd = Command::new("gh");
        cmd.args(["pr", "merge", pr_number]);

        match method {
            "squash" => {
                cmd.arg("--squash");
            }
            "rebase" => {
                cmd.arg("--rebase");
            }
            _ => {}
        }

        if delete_branch {
            cmd.arg("--delete-branch");
        }

        let output = cmd
            .output()
            .map_err(|e| onicode_core::CoreError::ToolError {
                tool: "gh_pr_merge".to_string(),
                message: format!("Failed to execute gh CLI: {}", e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(ToolOutput::success(format!(
                "PR #{} merged successfully\n{}",
                pr_number, stdout
            )))
        } else {
            Err(onicode_core::CoreError::ToolError {
                tool: "gh_pr_merge".to_string(),
                message: format!("gh pr merge failed: {}", stderr),
            })
        }
    }
}

pub struct GhPrStatusTool;

#[async_trait]
impl Tool for GhPrStatusTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "gh_pr_status".to_string(),
            description: "Check the status of a GitHub Pull Request".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["pr_number"],
                "properties": {
                    "pr_number": { "type": "string", "description": "PR number to check" },
                }
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput, onicode_core::CoreError> {
        let pr_number =
            input
                .get_str("pr_number")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "gh_pr_status".to_string(),
                    message: "pr_number is required".to_string(),
                })?;

        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                pr_number,
                "--json",
                "state,statusChecks,title,url,body",
            ])
            .output()
            .map_err(|e| onicode_core::CoreError::ToolError {
                tool: "gh_pr_status".to_string(),
                message: format!("Failed to execute gh CLI: {}", e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(ToolOutput::success(stdout.to_string()))
        } else {
            Err(onicode_core::CoreError::ToolError {
                tool: "gh_pr_status".to_string(),
                message: format!("gh pr view failed: {}", stderr),
            })
        }
    }
}

pub struct GhPrChecksTool;

#[async_trait]
impl Tool for GhPrChecksTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "gh_pr_checks".to_string(),
            description: "Check CI status for a GitHub Pull Request".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["pr_number"],
                "properties": {
                    "pr_number": { "type": "string", "description": "PR number to check" },
                }
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput, onicode_core::CoreError> {
        let pr_number =
            input
                .get_str("pr_number")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "gh_pr_checks".to_string(),
                    message: "pr_number is required".to_string(),
                })?;

        let output = Command::new("gh")
            .args(["pr", "checks", pr_number])
            .output()
            .map_err(|e| onicode_core::CoreError::ToolError {
                tool: "gh_pr_checks".to_string(),
                message: format!("Failed to execute gh CLI: {}", e),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(ToolOutput::success(stdout.to_string()))
        } else {
            Err(onicode_core::CoreError::ToolError {
                tool: "gh_pr_checks".to_string(),
                message: format!("gh pr checks failed: {}", stderr),
            })
        }
    }
}
