use async_trait::async_trait;
use onicode_core::{Tool, ToolInfo, ToolInput, ToolOutput};
use serde_json::json;

pub struct TaskTool;

#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> String {
        "task".to_string()
    }

    fn description(&self) -> String {
        "Delegate a task to a specialized subagent. Use this to parallelize work or leverage specialized capabilities.".to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Name of the subagent to use (e.g., 'explore' for codebase exploration, 'general' for general tasks)",
                    "enum": ["explore", "general"]
                },
                "task": {
                    "type": "string",
                    "description": "The task description to give to the subagent. Be specific and include all necessary context."
                },
                "description": {
                    "type": "string",
                    "description": "A short summary of the task for display purposes"
                }
            },
            "required": ["agent", "task"]
        })
    }

    async fn execute(&self, _input: ToolInput) -> Result<ToolOutput, String> {
        Err("The 'task' tool is handled internally by the agent loop and should not be executed directly".to_string())
    }
}
