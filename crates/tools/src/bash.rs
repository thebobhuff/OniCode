use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;
use tracing::debug;

pub struct BashTool {
    working_directory: String,
    timeout_secs: u64,
}

impl BashTool {
    pub fn new(working_directory: String, timeout_secs: u64) -> Self {
        Self {
            working_directory,
            timeout_secs,
        }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Bash".to_string(),
            description: "Execute a bash command in the terminal. Supports pipes, redirects, and environment variables. Use with caution.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 30)"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> onicode_core::Result<ToolOutput> {
        let command =
            input
                .get_str("command")
                .ok_or_else(|| onicode_core::CoreError::ToolError {
                    tool: "Bash".to_string(),
                    message: "Missing 'command' parameter".to_string(),
                })?;

        let timeout = input.get_i64("timeout").unwrap_or(self.timeout_secs as i64) as u64;

        debug!(command, timeout, "Executing bash command");

        #[cfg(windows)]
        let mut cmd = {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        };

        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = tokio::process::Command::new("bash");
            c.arg("-c").arg(command);
            c
        };

        cmd.current_dir(&self.working_directory);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output =
            tokio::time::timeout(std::time::Duration::from_secs(timeout), cmd.output()).await;

        match output {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    Ok(ToolOutput::success(if stdout.is_empty() {
                        stderr
                    } else {
                        format!("{stdout}\n{stderr}")
                    }))
                } else {
                    let code = output.status.code().unwrap_or(-1);
                    Ok(ToolOutput::error(format!(
                        "Command exited with code {code}\n{stderr}{stdout}"
                    )))
                }
            }
            Ok(Err(e)) => Err(onicode_core::CoreError::ToolError {
                tool: "Bash".to_string(),
                message: format!("Command execution failed: {e}"),
            }),
            Err(_) => Err(onicode_core::CoreError::ToolError {
                tool: "Bash".to_string(),
                message: format!("Command timed out after {timeout}s"),
            }),
        }
    }
}
