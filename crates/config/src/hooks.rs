use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    ToolError,
    Notification,
    SessionStart,
    SessionEnd,
    UserPrompt,
    AgentResponse,
}

impl std::fmt::Display for HookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookEvent::PreToolUse => write!(f, "PreToolUse"),
            HookEvent::PostToolUse => write!(f, "PostToolUse"),
            HookEvent::ToolError => write!(f, "ToolError"),
            HookEvent::Notification => write!(f, "Notification"),
            HookEvent::SessionStart => write!(f, "SessionStart"),
            HookEvent::SessionEnd => write!(f, "SessionEnd"),
            HookEvent::UserPrompt => write!(f, "UserPrompt"),
            HookEvent::AgentResponse => write!(f, "AgentResponse"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookHandler {
    Command { command: String },
    Notification { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub event: HookEvent,
    #[serde(default)]
    pub matcher: Option<String>,
    pub handler: HookHandler,
}

impl Hook {
    pub fn matches_tool(&self, tool_name: &str) -> bool {
        match &self.matcher {
            Some(pattern) => {
                if let Ok(re) = regex::Regex::new(pattern) {
                    re.is_match(tool_name)
                } else {
                    pattern == tool_name
                }
            }
            None => true,
        }
    }

    pub async fn execute(&self, context: &HookContext) -> std::result::Result<String, String> {
        match &self.handler {
            HookHandler::Command { command } => {
                let command_with_vars = self.interpolate(command, context);

                let output = tokio::process::Command::new("bash")
                    .arg("-c")
                    .arg(&command_with_vars)
                    .output()
                    .await
                    .map_err(|e| format!("Failed to execute hook: {e}"))?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    Ok(stdout.to_string())
                } else {
                    Err(format!("Hook failed: {stderr}"))
                }
            }
            HookHandler::Notification { message } => Ok(self.interpolate(message, context)),
        }
    }

    fn interpolate(&self, template: &str, context: &HookContext) -> String {
        template
            .replace("$COMMAND", &context.command)
            .replace("$FILE", &context.file_path)
            .replace("$MESSAGE", &context.message)
            .replace("$TOOL", &context.tool_name)
            .replace("$SESSION", &context.session_id)
            .replace("$OUTPUT", &context.output)
    }
}

#[derive(Debug, Clone, Default)]
pub struct HookContext {
    pub command: String,
    pub file_path: String,
    pub message: String,
    pub tool_name: String,
    pub session_id: String,
    pub output: String,
}
