use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, Result as SqliteResult};
use tracing::debug;

use crate::{
    message::{Message, MessageRole},
    session::Session,
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    working_directory TEXT NOT NULL,
    model TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    total_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tool_calls INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    tool_call_id TEXT,
    tool_call_name TEXT,
    tool_call_input TEXT,
    tool_result_content TEXT,
    tool_result_is_error INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at DESC);
"#;

pub struct SessionStore {
    conn: Connection,
}

impl SessionStore {
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        let dir = data_dir.unwrap_or_else(|| {
            dirs::data_local_dir()
                .map(|d| d.join("onicode").join("sessions"))
                .unwrap_or_else(|| PathBuf::from(".onicode/sessions"))
        });

        std::fs::create_dir_all(&dir).ok();

        let db_path = dir.join("sessions.db");
        let conn = Connection::open(&db_path).expect("Failed to open session database");

        conn.execute_batch(SCHEMA)
            .expect("Failed to create session schema");

        debug!(path = %db_path.display(), "Session store initialized");

        Self { conn }
    }

    pub fn save_session(&self, session: &Session) -> SqliteResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions (id, title, working_directory, model, created_at, updated_at, total_input_tokens, total_output_tokens, total_tool_calls)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                session.id.to_string(),
                session.metadata.tags.first().cloned().unwrap_or_default(),
                session.working_directory.clone(),
                session.metadata.model.clone(),
                session.metadata.created_at.timestamp(),
                session.metadata.updated_at.timestamp(),
                session.metadata.total_input_tokens as i64,
                session.metadata.total_output_tokens as i64,
                session.metadata.total_tool_calls as i64,
            ),
        )?;

        for msg in &session.messages {
            let (tool_call_id, tool_call_name, tool_call_input) =
                if let Some(ref tc) = msg.tool_calls {
                    if let Some(first) = tc.first() {
                        (
                            Some(first.id.clone()),
                            Some(first.name.clone()),
                            Some(first.input.to_string()),
                        )
                    } else {
                        (None, None, None)
                    }
                } else {
                    (None, None, None)
                };

            let (tool_result_content, tool_result_is_error) = if let Some(ref tr) = msg.tool_result
            {
                (Some(tr.content.clone()), tr.is_error as i64)
            } else {
                (None, 0)
            };

            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
                MessageRole::Tool => "tool",
            };

            self.conn.execute(
                "INSERT INTO messages (session_id, role, content, tool_call_id, tool_call_name, tool_call_input, tool_result_content, tool_result_is_error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                (
                    session.id.to_string(),
                    role,
                    msg.content.clone(),
                    tool_call_id,
                    tool_call_name,
                    tool_call_input,
                    tool_result_content,
                    tool_result_is_error,
                    chrono::Utc::now().timestamp(),
                ),
            )?;
        }

        Ok(())
    }

    pub fn load_session(&self, session_id: &str) -> SqliteResult<Option<Session>> {
        let session: Option<(String, String, String, String, i64, i64, i64, i64, i64)> = self
            .conn
            .query_row(
                "SELECT id, title, working_directory, model, created_at, updated_at, total_input_tokens, total_output_tokens, total_tool_calls FROM sessions WHERE id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            id,
            _title,
            working_directory,
            model,
            created_at,
            updated_at,
            total_input,
            total_output,
            total_tools,
        )) = session
        else {
            return Ok(None);
        };

        let mut session = Session::new(working_directory, model);
        session.id = id.parse().unwrap_or(session.id);
        session.metadata.total_input_tokens = total_input as u64;
        session.metadata.total_output_tokens = total_output as u64;
        session.metadata.total_tool_calls = total_tools as u64;
        session.metadata.created_at =
            chrono::DateTime::from_timestamp(created_at, 0).unwrap_or_else(chrono::Utc::now);
        session.metadata.updated_at =
            chrono::DateTime::from_timestamp(updated_at, 0).unwrap_or_else(chrono::Utc::now);

        let mut stmt = self.conn.prepare(
            "SELECT role, content, tool_call_id, tool_call_name, tool_call_input, tool_result_content, tool_result_is_error FROM messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;

        let messages = stmt.query_map([session_id.to_string()], |row| {
            let role: String = row.get(0)?;
            let content: String = row.get(1)?;
            let tool_call_id: Option<String> = row.get(2)?;
            let tool_call_name: Option<String> = row.get(3)?;
            let tool_call_input: Option<String> = row.get(4)?;
            let tool_result_content: Option<String> = row.get(5)?;
            let tool_result_is_error: i64 = row.get(6)?;

            let msg_role = match role.as_str() {
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                "tool" => MessageRole::Tool,
                _ => MessageRole::User,
            };

            let tc_id = tool_call_id.clone();
            let mut msg = match msg_role {
                MessageRole::User => Message::user(content),
                MessageRole::Assistant => Message::assistant(content),
                MessageRole::System => Message::system(content),
                MessageRole::Tool => Message::tool_result(
                    tc_id.unwrap_or_default(),
                    tool_result_content.clone().unwrap_or_default(),
                    tool_result_is_error != 0,
                ),
            };

            if let (Some(id), Some(name), Some(input)) =
                (tool_call_id, tool_call_name, tool_call_input)
            {
                if let Ok(input_json) = serde_json::from_str(&input) {
                    msg.tool_calls = Some(vec![crate::message::ToolCall {
                        id,
                        name,
                        input: input_json,
                    }]);
                }
            }

            if let Some(result_content) = tool_result_content {
                msg.tool_result = Some(crate::message::ToolResult {
                    tool_call_id: String::new(),
                    content: result_content,
                    is_error: tool_result_is_error != 0,
                });
            }

            Ok(msg)
        })?;

        for msg in messages {
            if let Ok(msg) = msg {
                session.messages.push(msg);
            }
        }

        Ok(Some(session))
    }

    pub fn list_sessions(&self) -> SqliteResult<Vec<(String, String, String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, model, updated_at FROM sessions ORDER BY updated_at DESC LIMIT 50",
        )?;

        let sessions = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(sessions)
    }

    pub fn delete_session(&self, session_id: &str) -> SqliteResult<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
        Ok(())
    }
}
