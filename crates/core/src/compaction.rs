use std::collections::HashSet;

use crate::{
    message::{Message, MessageRole},
    session::Session,
};

#[derive(Debug, Clone)]
pub struct FileOperation {
    pub tool: String,
    pub file: String,
    pub operation: FileOpType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileOpType {
    Read,
    Write,
    Edit,
}

#[derive(Debug, Clone)]
pub struct CompactionSummary {
    pub content: String,
    pub file_operations: Vec<FileOperation>,
    pub kept_message_count: usize,
}

pub fn extract_file_operations(messages: &[Message]) -> Vec<FileOperation> {
    let mut ops = Vec::new();

    for msg in messages {
        if msg.role != MessageRole::Tool {
            continue;
        }

        if let Some(ref tool_result) = msg.tool_result {
            let tool_name = tool_result.tool_call_id.split('_').next().unwrap_or("");

            let file = extract_file_from_result(&tool_result.content);

            if let Some(f) = file {
                let op_type = match tool_name {
                    "read" => FileOpType::Read,
                    "write" => FileOpType::Write,
                    "edit" => FileOpType::Edit,
                    _ => continue,
                };

                ops.push(FileOperation {
                    tool: tool_name.to_string(),
                    file: f,
                    operation: op_type,
                });
            }
        }

        if let Some(ref tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                let file = extract_file_from_input(&tc.name, &tc.input);

                if let Some(f) = file {
                    let op_type = match tc.name.as_str() {
                        "read" => FileOpType::Read,
                        "write" => FileOpType::Write,
                        "edit" => FileOpType::Edit,
                        _ => continue,
                    };

                    ops.push(FileOperation {
                        tool: tc.name.clone(),
                        file: f,
                        operation: op_type,
                    });
                }
            }
        }
    }

    ops
}

fn extract_file_from_result(content: &str) -> Option<String> {
    if content.contains("path:") || content.contains("file:") {
        for line in content.lines() {
            if line.starts_with("path:") || line.starts_with("file:") {
                return Some(
                    line.splitn(2, ':')
                        .nth(1)?
                        .trim()
                        .trim_matches('"')
                        .to_string(),
                );
            }
        }
    }

    if content.starts_with('/') || content.contains('/') {
        for word in content.split_whitespace() {
            if word.contains('/')
                || word.ends_with(".rs")
                || word.ends_with(".toml")
                || word.ends_with(".md")
            {
                return Some(word.trim_matches('"').to_string());
            }
        }
    }

    None
}

fn extract_file_from_input(name: &str, input: &serde_json::Value) -> Option<String> {
    match name {
        "read" | "write" | "edit" => input["path"]
            .as_str()
            .map(String::from)
            .or_else(|| input["file_path"].as_str().map(String::from)),
        _ => None,
    }
}

pub fn compact_session(session: &mut Session, keep_recent: usize) -> Option<CompactionSummary> {
    if session.messages.len() <= keep_recent + 2 {
        return None;
    }

    let messages_to_compact = &session.messages[..session.messages.len() - keep_recent];

    let file_ops = extract_file_operations(messages_to_compact);

    let mut summary_parts = Vec::new();

    summary_parts.push(format!(
        "Compacted {} messages into this summary.",
        messages_to_compact.len()
    ));

    if !file_ops.is_empty() {
        let mut files_read: HashSet<&str> = HashSet::new();
        let mut files_modified: HashSet<&str> = HashSet::new();

        for op in &file_ops {
            match op.operation {
                FileOpType::Read => {
                    files_read.insert(&op.file);
                }
                FileOpType::Write | FileOpType::Edit => {
                    files_modified.insert(&op.file);
                }
            }
        }

        if !files_read.is_empty() {
            let files: Vec<String> = files_read.iter().map(|s| s.to_string()).collect();
            summary_parts.push(format!("Files read: {}", files.join(", ")));
        }

        if !files_modified.is_empty() {
            let files: Vec<String> = files_modified.iter().map(|s| s.to_string()).collect();
            summary_parts.push(format!("Files modified: {}", files.join(", ")));
        }
    }

    let summary = summary_parts.join("\n");

    session.add_message(Message::system(&summary));

    let kept_count = keep_recent;

    session
        .messages
        .drain(..session.messages.len() - keep_recent - 1);

    Some(CompactionSummary {
        content: summary,
        file_operations: file_ops,
        kept_message_count: kept_count,
    })
}

pub fn should_compact(session: &Session, threshold: usize) -> bool {
    session.messages.len() > threshold
}
