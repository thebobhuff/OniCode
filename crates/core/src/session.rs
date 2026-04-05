use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::Result, message::Message};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub messages: Vec<Message>,
    pub metadata: SessionMetadata,
    pub working_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub model: String,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tool_calls: u64,
    pub tags: Vec<String>,
}

impl Session {
    pub fn new(working_directory: String, model: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            messages: Vec::new(),
            metadata: SessionMetadata {
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                model,
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_tool_calls: 0,
                tags: Vec::new(),
            },
            working_directory,
        }
    }

    pub fn add_message(&mut self, msg: Message) {
        self.messages.push(msg);
        self.metadata.updated_at = chrono::Utc::now();
    }

    pub fn last_n_messages(&self, n: usize) -> &[Message] {
        let len = self.messages.len();
        if len <= n {
            &self.messages
        } else {
            &self.messages[len - n..]
        }
    }

    pub fn compact(&mut self, keep: usize) {
        if self.messages.len() > keep {
            let removed = self.messages.len() - keep;
            self.messages.drain(..removed);
        }
    }

    pub fn token_count(&self) -> u64 {
        self.metadata.total_input_tokens + self.metadata.total_output_tokens
    }

    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id,
            message_count: self.messages.len(),
            model: self.metadata.model.clone(),
            total_tokens: self.token_count(),
            working_directory: self.working_directory.clone(),
            created_at: self.metadata.created_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: Uuid,
    pub message_count: usize,
    pub model: String,
    pub total_tokens: u64,
    pub working_directory: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct SessionStore {
    sessions: HashMap<Uuid, Session>,
    active_session: Option<Uuid>,
    storage_path: std::path::PathBuf,
}

impl SessionStore {
    pub fn new(storage_path: std::path::PathBuf) -> Self {
        std::fs::create_dir_all(&storage_path).ok();
        Self {
            sessions: HashMap::new(),
            active_session: None,
            storage_path,
        }
    }

    pub fn create(&mut self, working_directory: String, model: String) -> &Session {
        let session = Session::new(working_directory, model);
        let id = session.id;
        self.sessions.insert(id, session);
        self.active_session = Some(id);
        self.sessions.get(&id).unwrap()
    }

    pub fn active(&self) -> Option<&Session> {
        self.active_session.and_then(|id| self.sessions.get(&id))
    }

    pub fn active_mut(&mut self) -> Option<&mut Session> {
        self.active_session
            .and_then(|id| self.sessions.get_mut(&id))
    }

    pub fn get(&self, id: Uuid) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn list(&self) -> Vec<SessionSummary> {
        self.sessions.values().map(|s| s.summary()).collect()
    }

    pub fn save(&self, id: Uuid) -> Result<()> {
        if let Some(session) = self.sessions.get(&id) {
            let path = self.storage_path.join(format!("{}.json", id));
            let json = serde_json::to_string_pretty(session)?;
            std::fs::write(path, json)?;
        }
        Ok(())
    }

    pub fn load(&mut self, id: Uuid) -> Result<()> {
        let path = self.storage_path.join(format!("{}.json", id));
        let json = std::fs::read_to_string(path)?;
        let session: Session = serde_json::from_str(&json)?;
        self.sessions.insert(id, session);
        self.active_session = Some(id);
        Ok(())
    }
}
