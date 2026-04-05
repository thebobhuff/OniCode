use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResult>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parts: Vec<MessagePart>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageFinish {
    ToolCalls,
    Stop,
    EndTurn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessagePart {
    Text {
        content: String,
    },
    Tool {
        tool: String,
        input: serde_json::Value,
        state: ToolPartState,
    },
    Subtask {
        agent: String,
        task: String,
        session_id: Option<String>,
        result: Option<String>,
    },
    Compaction {
        summary: String,
        file_operations: Vec<FileOp>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOp {
    pub tool: String,
    pub file: String,
    pub operation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ToolPartState {
    Pending,
    Running,
    Completed { output: String },
    Error { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn tool_result(tool_call_id: String, content: String, is_error: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Tool,
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                tool_call_id,
                content,
                is_error,
            }),
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn subtask(agent: String, task: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: format!("@{agent}: {task}"),
            tool_calls: None,
            tool_result: None,
            parts: vec![MessagePart::Subtask {
                agent,
                task,
                session_id: None,
                result: None,
            }],
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_part(mut self, part: MessagePart) -> Self {
        self.parts.push(part);
        self
    }
}
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn assistant_with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn tool_result(tool_call_id: String, content: String, is_error: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::Tool,
            content: String::new(),
            tool_calls: None,
            tool_result: Some(ToolResult {
                tool_call_id,
                content,
                is_error,
            }),
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_result: None,
            parts: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn subtask(agent: String, task: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            role: MessageRole::User,
            content: format!("@{agent}: {task}"),
            tool_calls: None,
            tool_result: None,
            parts: vec![MessagePart::Subtask {
                agent,
                task,
                session_id: None,
                result: None,
            }],
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn with_part(mut self, part: MessagePart) -> Self {
        self.parts.push(part);
        self
    }
}
