use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::{
    error::Result,
    tool::{Tool, ToolInfo, ToolInput, ToolOutput},
};

#[derive(Debug, Clone)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug)]
pub struct QuestionRequest {
    pub question: String,
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multiple: bool,
}

#[derive(Debug)]
pub struct QuestionResponse {
    pub answers: Vec<String>,
}

type QuestionChannel = (QuestionRequest, oneshot::Sender<QuestionResponse>);

#[derive(Clone)]
pub struct QuestionBridge {
    tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<QuestionChannel>>>>,
}

impl QuestionBridge {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_sender(&self, tx: tokio::sync::mpsc::UnboundedSender<QuestionChannel>) {
        *self.tx.lock() = Some(tx);
    }

    pub fn get_sender(&self) -> Option<tokio::sync::mpsc::UnboundedSender<QuestionChannel>> {
        self.tx.lock().clone()
    }

    pub fn is_configured(&self) -> bool {
        self.tx.lock().is_some()
    }
}

pub struct QuestionTool {
    bridge: QuestionBridge,
}

impl QuestionTool {
    pub fn new(bridge: QuestionBridge) -> Self {
        Self { bridge }
    }

    async fn ask_question(&self, request: QuestionRequest) -> Result<QuestionResponse> {
        let tx = self
            .bridge
            .get_sender()
            .ok_or_else(|| crate::error::CoreError::ToolError {
                tool: "question".to_string(),
                message: "Question tool not connected to UI".to_string(),
            })?;

        let (response_tx, response_rx) = oneshot::channel();

        tx.send((request, response_tx))
            .map_err(|_| crate::error::CoreError::ToolError {
                tool: "question".to_string(),
                message: "Failed to send question to UI".to_string(),
            })?;

        let response = response_rx
            .await
            .map_err(|_| crate::error::CoreError::ToolError {
                tool: "question".to_string(),
                message: "Question was cancelled or timed out".to_string(),
            })?;

        Ok(response)
    }
}

#[async_trait]
impl Tool for QuestionTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "question".to_string(),
            description: "Ask the user a question during execution. Use this to gather preferences, clarify ambiguous instructions, get decisions on implementation choices, or offer choices about what direction to take. Answers are returned as arrays of labels; set `multiple: true` to allow selecting more than one option. If you recommend a specific option, make that the first option in the list and add \"(Recommended)\" at the end of the label".to_string(),
            parameters: json!({
                "type": "object",
                "required": ["question", "header", "options"],
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user"
                    },
                    "header": {
                        "type": "string",
                        "description": "Very short label for the question (max 30 chars)"
                    },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["label", "description"],
                            "properties": {
                                "label": {
                                    "type": "string",
                                    "description": "Display text for the option (1-5 words, concise)"
                                },
                                "description": {
                                    "type": "string",
                                    "description": "Explanation of what this option means"
                                }
                            }
                        },
                        "description": "Available choices for the user"
                    },
                    "multiple": {
                        "type": "boolean",
                        "description": "Allow the user to select multiple options (default: false)"
                    }
                }
            }),
        }
    }

    async fn execute(&self, input: ToolInput) -> Result<ToolOutput> {
        let question =
            input
                .get_str("question")
                .ok_or_else(|| crate::error::CoreError::ToolError {
                    tool: "question".to_string(),
                    message: "Missing required parameter: question".to_string(),
                })?;

        let header = input.get_str("header").unwrap_or("Question");

        let multiple = input.get_bool("multiple").unwrap_or(false);

        let options: Vec<QuestionOption> = input
            .parameters
            .get("options")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|opt| {
                        Some(QuestionOption {
                            label: opt["label"].as_str()?.to_string(),
                            description: opt["description"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if options.is_empty() {
            return Err(crate::error::CoreError::ToolError {
                tool: "question".to_string(),
                message: "At least one option is required".to_string(),
            });
        }

        let request = QuestionRequest {
            question: question.to_string(),
            header: header.to_string(),
            options,
            multiple,
        };

        let response = self.ask_question(request).await?;

        let answers_json = json!({
            "answers": response.answers
        });

        Ok(ToolOutput::success(answers_json.to_string()))
    }
}
