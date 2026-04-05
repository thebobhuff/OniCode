use std::{collections::HashMap, sync::Arc};

use onicode_config::PermissionMode;
use parking_lot::RwLock;
use tokio::sync::watch;

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    Allow,
    Deny,
    Pending,
}

#[derive(Debug, Clone)]
pub struct PermissionEntry {
    pub mode: PermissionMode,
    pub decision: PermissionDecision,
}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub tool: String,
    pub input: String,
    pub reason: PendingPermissionReason,
    pub decision_tx: watch::Sender<PermissionDecision>,
    pub decision_rx: watch::Receiver<PermissionDecision>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PendingPermissionReason {
    Normal,
    DoomLoop { count: usize },
}

pub struct PermissionManager {
    permissions: Arc<RwLock<HashMap<String, PermissionEntry>>>,
    default_mode: PermissionMode,
    pending: Arc<RwLock<Option<PendingPermission>>>,
}

impl PermissionManager {
    pub fn new(default_mode: PermissionMode) -> Self {
        Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
            default_mode,
            pending: Arc::new(RwLock::new(None)),
        }
    }

    pub fn check(&self, tool_name: &str) -> PermissionDecision {
        let perms = self.permissions.read();

        if let Some(entry) = perms.get(tool_name) {
            return entry.decision.clone();
        }

        match self.default_mode {
            PermissionMode::Allow => PermissionDecision::Allow,
            PermissionMode::Deny => PermissionDecision::Deny,
            PermissionMode::Ask => PermissionDecision::Pending,
        }
    }

    pub fn set_permission(
        &self,
        tool_name: &str,
        mode: PermissionMode,
        decision: PermissionDecision,
    ) {
        let mut perms = self.permissions.write();
        perms.insert(tool_name.to_string(), PermissionEntry { mode, decision });
    }

    pub fn set_default_mode(&mut self, mode: PermissionMode) {
        self.default_mode = mode;
    }

    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Allow)
    }

    pub fn is_tool_denied(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Deny)
    }

    pub fn needs_approval(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Pending)
    }

    pub fn request_pending_permission(
        &self,
        tool: String,
        input: String,
        reason: PendingPermissionReason,
    ) -> watch::Receiver<PermissionDecision> {
        let (decision_tx, decision_rx) = watch::channel(PermissionDecision::Pending);
        let mut pending = self.pending.write();
        *pending = Some(PendingPermission {
            tool,
            input,
            reason,
            decision_tx,
            decision_rx: decision_rx.clone(),
        });
        decision_rx
    }

    pub fn respond_pending_permission(&self, decision: PermissionDecision) {
        let mut pending = self.pending.write();
        if let Some(p) = pending.as_ref() {
            let _ = p.decision_tx.send(decision);
        }
        *pending = None;
    }

    pub fn get_pending_permission(&self) -> Option<PendingPermission> {
        self.pending.read().clone()
    }

    pub fn clear_pending(&self) {
        let mut pending = self.pending.write();
        *pending = None;
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEntry {
    pub mode: PermissionMode,
    pub decision: PermissionDecision,
}

#[derive(Debug, Clone)]
pub struct PendingPermission {
    pub tool: String,
    pub input: String,
    pub reason: PendingPermissionReason,
    pub decision_tx: watch::Sender<PermissionDecision>,
    pub decision_rx: watch::Receiver<PermissionDecision>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PendingPermissionReason {
    Normal,
    DoomLoop { count: usize },
}

pub struct PermissionManager {
    permissions: Arc<RwLock<HashMap<String, PermissionEntry>>>,
    default_mode: PermissionMode,
    pending: Arc<RwLock<Option<PendingPermission>>>,
}

impl PermissionManager {
    pub fn new(default_mode: PermissionMode) -> Self {
        Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
            default_mode,
            pending: Arc::new(RwLock::new(None)),
        }
    }

    pub fn check(&self, tool_name: &str) -> PermissionDecision {
        let perms = self.permissions.read();

        if let Some(entry) = perms.get(tool_name) {
            return entry.decision.clone();
        }

        match self.default_mode {
            PermissionMode::Allow => PermissionDecision::Allow,
            PermissionMode::Deny => PermissionDecision::Deny,
            PermissionMode::Ask => PermissionDecision::Pending,
        }
    }

    pub fn set_permission(
        &self,
        tool_name: &str,
        mode: PermissionMode,
        decision: PermissionDecision,
    ) {
        let mut perms = self.permissions.write();
        perms.insert(tool_name.to_string(), PermissionEntry { mode, decision });
    }

    pub fn set_default_mode(&mut self, mode: PermissionMode) {
        self.default_mode = mode;
    }

    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Allow)
    }

    pub fn is_tool_denied(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Deny)
    }

    pub fn needs_approval(&self, tool_name: &str) -> bool {
        matches!(self.check(tool_name), PermissionDecision::Pending)
    }

    pub fn request_pending_permission(
        &self,
        tool: String,
        input: String,
        reason: PendingPermissionReason,
    ) -> watch::Receiver<PermissionDecision> {
        let (decision_tx, decision_rx) = watch::channel(PermissionDecision::Pending);
        let mut pending = self.pending.write();
        *pending = Some(PendingPermission {
            tool,
            input,
            reason,
            decision_tx,
            decision_rx: decision_rx.clone(),
        });
        decision_rx
    }

    pub fn respond_pending_permission(&self, decision: PermissionDecision) {
        let mut pending = self.pending.write();
        if let Some(p) = pending.as_ref() {
            let _ = p.decision_tx.send(decision);
        }
        *pending = None;
    }

    pub fn get_pending_permission(&self) -> Option<PendingPermission> {
        self.pending.read().clone()
    }

    pub fn clear_pending(&self) {
        let mut pending = self.pending.write();
        *pending = None;
    }
}
