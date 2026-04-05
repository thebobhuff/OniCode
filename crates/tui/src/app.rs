use crossterm::event::{Event, EventStream};
use onicode_config::Hook;
use onicode_core::{
    LlmClient, PermissionManager, Session, SessionStore, ToolRegistry,
    agent::{Agent, AgentEvent, AgentMode},
    question::{QuestionBridge, QuestionRequest, QuestionResponse},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

use crate::{components::StatusBar, events::TuiEvent, renderer, theme::Theme};

#[derive(Debug)]
pub enum AppMessage {
    User(String),
    Assistant(String),
    ToolCall { name: String, input: String },
    ToolResult { name: String, is_error: bool },
    Error(String),
    Thinking(String),
    System(String),
}

impl AppMessage {
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Local> {
        chrono::Local::now()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuestionMode {
    None,
    Selecting(usize),
    Typing,
}

pub struct CommandDef {
    pub name: &'static str,
    pub description: &'static str,
    pub usage: &'static str,
}

pub struct CustomCommand {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub prompt: String,
}

pub fn load_custom_commands(workspace_root: &std::path::Path) -> Vec<CustomCommand> {
    let commands_dir = workspace_root.join(".onicode").join("commands");
    let mut commands = Vec::new();

    if !commands_dir.exists() {
        return commands;
    }

    for entry in std::fs::read_dir(&commands_dir).ok().into_iter().flatten() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let mut description = String::new();
                    let mut prompt = String::new();
                    let mut in_prompt = false;

                    for line in content.lines() {
                        if line.starts_with("## ") && line.contains("Prompt") {
                            in_prompt = true;
                            continue;
                        }
                        if line.starts_with("## ") {
                            in_prompt = false;
                            continue;
                        }
                        if line.starts_with("## Usage") {
                            continue;
                        }
                        if line.starts_with("# ") && description.is_empty() {
                            description = line.trim_start_matches("# ").trim().to_string();
                            continue;
                        }
                        if in_prompt {
                            if !prompt.is_empty() {
                                prompt.push('\n');
                            }
                            prompt.push_str(line);
                        }
                    }

                    if prompt.is_empty() {
                        prompt = content.clone();
                    }

                    commands.push(CustomCommand {
                        name,
                        description,
                        usage: format!(
                            "/{}",
                            path.file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default()
                        ),
                        prompt: prompt.trim().to_string(),
                    });
                }
            }
        }
    }

    commands
}

pub const COMMANDS: &[CommandDef] = &[
    CommandDef {
        name: "help",
        description: "Show available commands",
        usage: "/help",
    },
    CommandDef {
        name: "clear",
        description: "Clear chat history",
        usage: "/clear",
    },
    CommandDef {
        name: "model",
        description: "Show or change the current model",
        usage: "/model <name>",
    },
    CommandDef {
        name: "provider",
        description: "Show or change the current provider",
        usage: "/provider <name>",
    },
    CommandDef {
        name: "mode",
        description: "Switch agent mode (build/plan)",
        usage: "/mode <build|plan>",
    },
    CommandDef {
        name: "compact",
        description: "Manually compact context",
        usage: "/compact",
    },
    CommandDef {
        name: "doctor",
        description: "Show configuration status",
        usage: "/doctor",
    },
    CommandDef {
        name: "setup",
        description: "Re-run setup wizard",
        usage: "/setup",
    },
    CommandDef {
        name: "exit",
        description: "Exit OniCode",
        usage: "/exit",
    },
    CommandDef {
        name: "attach",
        description: "Attach a file to your message",
        usage: "/attach <path>",
    },
    CommandDef {
        name: "attachments",
        description: "List current attachments",
        usage: "/attachments",
    },
    CommandDef {
        name: "clear-attachments",
        description: "Clear all attachments",
        usage: "/clear-attachments",
    },
];

#[derive(Debug, Clone)]
pub struct Attachment {
    pub path: String,
    pub kind: AttachmentKind,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AttachmentKind {
    Text,
    Image,
    Binary,
}

impl AttachmentKind {
    pub fn from_path(path: &str) -> Self {
        let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
        match ext.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
                AttachmentKind::Image
            }
            "txt" | "md" | "rs" | "toml" | "json" | "yaml" | "yml" | "js" | "ts" | "py" | "go"
            | "c" | "cpp" | "h" | "html" | "css" | "sh" | "bash" | "zsh" | "ps1" | "bat"
            | "log" | "csv" | "xml" | "ini" | "cfg" | "conf" | "gitignore" | "dockerfile"
            | "makefile" | "java" | "kt" | "swift" | "rb" | "php" | "vue" | "jsx" | "tsx"
            | "svelte" | "astro" | "markdown" => AttachmentKind::Text,
            _ => AttachmentKind::Binary,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            AttachmentKind::Text => "[FILE]",
            AttachmentKind::Image => "[IMG]",
            AttachmentKind::Binary => "[BIN]",
        }
    }
}

pub struct SubAgentCard {
    pub id: usize,
    pub name: String,
    pub task: String,
    pub status: SubAgentStatus,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubAgentStatus {
    Thinking,
    Working,
    Complete,
    Error(String),
}

pub struct App {
    pub theme: Theme,
    pub messages: Vec<AppMessage>,
    pub input_buffer: String,
    pub cursor_pos: usize,
    pub is_focused: bool,
    pub is_processing: bool,
    pub scroll_offset: u16,
    pub current_model: String,
    pub working_directory: String,
    pub status_bar: StatusBar,
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub question_rx: mpsc::UnboundedReceiver<(
        QuestionRequest,
        tokio::sync::oneshot::Sender<QuestionResponse>,
    )>,
    pub active: bool,
    pub pending_question: Option<(
        QuestionRequest,
        tokio::sync::oneshot::Sender<QuestionResponse>,
    )>,
    pub question_selected: usize,
    pub question_custom_input: String,
    pub question_mode: QuestionMode,
    pub agent_tx: mpsc::Sender<String>,
    pub session: Session,
    pub session_store: SessionStore,
    pub show_commands: bool,
    pub command_filter: String,
    pub attachments: Vec<Attachment>,
    pub file_picker_active: bool,
    pub file_picker_path: String,
    pub file_picker_entries: Vec<String>,
    pub file_picker_selected: usize,
    pub file_picker_offset: usize,
    pub context_used: u64,
    pub context_limit: u64,
    pub collapsed_blocks: std::collections::HashSet<usize>,
    pub session_title: String,
    pub tasks: Vec<Task>,
    pub custom_commands: Vec<CustomCommand>,
    pub streaming_buffer: String,
    pub subagents: Vec<SubAgentCard>,
    pub subagent_id_counter: usize,
    pub collapsed_subagents: std::collections::HashSet<usize>,
    pub pr_state: PrState,
    pub focused_button: usize,
    pub context_breakdown: ContextBreakdown,
}

#[derive(Debug, Clone, Default)]
pub struct ContextBreakdown {
    pub system_instructions: usize,
    pub project_context: usize,
    pub conversation: usize,
    pub attachments: usize,
}

impl ContextBreakdown {
    pub fn total(&self) -> usize {
        self.system_instructions + self.project_context + self.conversation + self.attachments
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrState {
    None,
    HasChanges { branch: String, changes: usize },
    PrCreated { pr_number: u64, url: String },
    MergeReady { pr_number: u64 },
    HasErrors { pr_number: u64, errors: String },
}

#[derive(Debug, Clone)]
pub struct Task {
    pub title: String,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Complete,
}

#[derive(Debug)]
pub enum AppEvent {
    Agent(AgentEvent),
    Question(
        QuestionRequest,
        tokio::sync::oneshot::Sender<QuestionResponse>,
    ),
}

pub struct AppConfig {
    pub client: Arc<LlmClient>,
    pub tools: Arc<ToolRegistry>,
    pub max_turns: usize,
    pub agent_mode: AgentMode,
    pub permissions: Arc<PermissionManager>,
    pub working_directory: String,
    pub question_bridge: QuestionBridge,
    pub hooks: Vec<Hook>,
}

use std::sync::Arc;

impl App {
    pub fn new(config: AppConfig) -> (Self, mpsc::Sender<String>) {
        let theme = Theme::dark();
        let (agent_event_tx, mut agent_event_rx) = mpsc::channel::<AgentEvent>(64);
        let (app_event_tx, app_event_rx) = mpsc::channel::<AppEvent>(128);
        let (question_tx, question_rx) = mpsc::unbounded_channel();
        let (agent_tx, mut agent_rx) = mpsc::channel::<String>(8);

        let question_bridge = config.question_bridge.clone();
        question_bridge.set_sender(question_tx);

        let session = Session::new(
            config.working_directory.clone(),
            config.client.config().model.clone(),
        );
        let session_store = SessionStore::new(None);

        let model = config.client.config().model.clone();
        let work_dir = config.working_directory.clone();
        let model_for_spawn = model.clone();
        let work_dir_for_spawn = work_dir.clone();

        tokio::spawn(async move {
            let hooks = config.hooks.clone();
            loop {
                tokio::select! {
                    Some(prompt) = agent_rx.recv() => {
                        let mut session = Session::new(work_dir_for_spawn.clone(), model_for_spawn.clone());

                        if let Some(ctx) = onicode_core::load_project_context(work_dir_for_spawn.as_ref()) {
                            session.add_message(onicode_core::Message::system(&ctx));
                        }

                        let agent = Agent::new(
                            config.client.clone(),
                            config.tools.clone(),
                            config.max_turns,
                            agent_event_tx.clone(),
                        )
                        .with_question_bridge(config.question_bridge.clone())
                        .with_mode(config.agent_mode.clone())
                        .with_permissions(config.permissions.clone())
                        .with_hooks(hooks.clone(), session.id.to_string());

                        let _ = agent.run(&mut session, &prompt).await;
                    }
                    Some(agent_event) = agent_event_rx.recv() => {
                        let _ = app_event_tx.send(AppEvent::Agent(agent_event)).await;
                    }
                }
            }
        });

        let app = Self {
            theme: theme.clone(),
            messages: vec![AppMessage::System(
                "OniCode — Open source terminal coding agent".into(),
            )],
            input_buffer: String::new(),
            cursor_pos: 0,
            is_focused: true,
            is_processing: false,
            scroll_offset: 0,
            current_model: model,
            working_directory: work_dir.clone(),
            status_bar: StatusBar {
                left: "NORMAL".into(),
                center: "".into(),
                right: "^K commands".into(),
            },
            event_rx: app_event_rx,
            question_rx,
            active: true,
            pending_question: None,
            question_selected: 0,
            question_custom_input: String::new(),
            question_mode: QuestionMode::None,
            agent_tx: agent_tx.clone(),
            session,
            session_store,
            show_commands: false,
            command_filter: String::new(),
            attachments: Vec::new(),
            file_picker_active: false,
            file_picker_path: String::new(),
            file_picker_entries: Vec::new(),
            file_picker_selected: 0,
            file_picker_offset: 0,
            context_used: 0,
            context_limit: 200_000,
            collapsed_blocks: std::collections::HashSet::new(),
            session_title: work_dir
                .split(std::path::MAIN_SEPARATOR)
                .last()
                .unwrap_or("Untitled")
                .to_string(),
            tasks: Vec::new(),
            custom_commands: Vec::new(),
            streaming_buffer: String::new(),
            subagents: Vec::new(),
            subagent_id_counter: 0,
            collapsed_subagents: std::collections::HashSet::new(),
            pr_state: PrState::None,
            focused_button: 0,
            context_breakdown: ContextBreakdown::default(),
        };

        (app, agent_tx)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut terminal = self.init_terminal()?;
        let mut event_stream = EventStream::new();

        loop {
            if !self.active {
                break;
            }

            tokio::select! {
                Some(app_event) = self.event_rx.recv() => {
                    match app_event {
                        AppEvent::Agent(agent_event) => {
                            self.handle_agent_event(agent_event);
                        }
                        AppEvent::Question(request, responder) => {
                            self.pending_question = Some((request, responder));
                            self.handle_incoming_question();
                        }
                    }
                }
                maybe_event = event_stream.next() => {
                    if let Some(Ok(event)) = maybe_event {
                        match event {
                            Event::Key(key) => {
                                let tui_event = TuiEvent::Key(key);
                                if tui_event.is_quit() {
                                    self.active = false;
                                    break;
                                }
                                self.handle_key(key);
                            }
                            Event::Resize(w, h) => {
                                let _ = TuiEvent::Resize(w, h);
                            }
                            Event::Mouse(mouse) => {
                                use crossterm::event::MouseEventKind;
                                match mouse.kind {
                                    MouseEventKind::ScrollUp => {
                                        self.scroll_offset = self.scroll_offset.saturating_sub(3);
                                    }
                                    MouseEventKind::ScrollDown => {
                                        self.scroll_offset += 3;
                                    }
                                    MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                        for (i, msg) in self.messages.iter().enumerate() {
                                            if matches!(msg, AppMessage::ToolCall { .. }) {
                                                if self.collapsed_blocks.contains(&i) {
                                                    self.collapsed_blocks.remove(&i);
                                                } else {
                                                    self.collapsed_blocks.insert(i);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Event::Paste(text) => {
                                if matches!(self.question_mode, QuestionMode::None) {
                                    self.input_buffer.insert_str(self.cursor_pos, &text);
                                    self.cursor_pos += text.len();
                                    self.show_commands = false;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            terminal.draw(|frame| renderer::render(frame, self))?;
        }

        self.cleanup_terminal()?;
        Ok(())
    }

    pub fn inject_system_context(&mut self, context: &str) {
        self.messages
            .insert(0, AppMessage::System(context.to_string()));

        // Estimate tokens: ~4 chars per token
        self.context_breakdown.system_instructions = context.len() / 4;
    }

    pub fn update_conversation_context(&mut self) {
        let mut conv_chars = 0;
        for msg in &self.messages {
            match msg {
                AppMessage::User(t) | AppMessage::Assistant(t) | AppMessage::Thinking(t) => {
                    conv_chars += t.len();
                }
                AppMessage::ToolCall { input, .. } => {
                    conv_chars += input.len();
                }
                AppMessage::ToolResult { .. } => {}
                _ => {}
            }
        }
        self.context_breakdown.conversation = conv_chars / 4;
    }

    pub fn handle_incoming_question(&mut self) {
        if let Some((ref req, _)) = self.pending_question {
            self.messages.push(AppMessage::System(format!(
                "❓ {}: {}",
                req.header, req.question
            )));
            for (i, opt) in req.options.iter().enumerate() {
                self.messages.push(AppMessage::System(format!(
                    "  [{i}] {} — {}",
                    opt.label, opt.description
                )));
            }
            self.messages.push(AppMessage::System(
                "  [t] Type custom answer | [0-9] Select | [Enter] Confirm".into(),
            ));
            self.question_mode = QuestionMode::Selecting(0);
            self.question_selected = 0;
            self.is_processing = true;
            self.status_bar.left = "QUESTION".into();
        }
    }

    pub fn answer_question_with_selection(&mut self) {
        let Some((req, responder)) = self.pending_question.take() else {
            return;
        };

        let answers = match self.question_mode {
            QuestionMode::Selecting(idx) if idx < req.options.len() => {
                vec![req.options[idx].label.clone()]
            }
            QuestionMode::Typing if !self.question_custom_input.is_empty() => {
                vec![self.question_custom_input.clone()]
            }
            _ => vec!["cancelled".to_string()],
        };

        let _ = responder.send(QuestionResponse { answers });
        self.question_mode = QuestionMode::None;
        self.question_custom_input.clear();
        self.is_processing = false;
        self.status_bar.left = "NORMAL".into();
        self.status_bar.center = "".into();
    }

    pub fn handle_pr_button(&mut self) {
        match &self.pr_state {
            PrState::HasChanges { branch, changes } => {
                self.messages.push(AppMessage::User(format!(
                    "Create a PR from branch '{}' with {} change(s).",
                    branch, changes
                )));
                self.is_processing = true;
                self.status_bar.left = "PROCESSING".into();
                self.status_bar.center = "Creating PR...".into();
                let _ = self
                    .agent_tx
                    .try_send(format!("Create a PR from branch '{}' with {} change(s). Review the diff, write a good commit message, and push to remote.", branch, changes));
            }
            PrState::PrCreated { pr_number, url } => {
                self.messages
                    .push(AppMessage::User(format!("Merge PR #{}.", pr_number)));
                self.is_processing = true;
                self.status_bar.left = "PROCESSING".into();
                self.status_bar.center = "Merging PR...".into();
                let _ = self
                    .agent_tx
                    .try_send(format!("Merge PR #{} at {}", pr_number, url));
            }
            PrState::MergeReady { pr_number } => {
                self.messages
                    .push(AppMessage::User(format!("Merge PR #{}.", pr_number)));
                self.is_processing = true;
                self.status_bar.left = "PROCESSING".into();
                self.status_bar.center = "Merging PR...".into();
                let _ = self
                    .agent_tx
                    .try_send(format!("Merge PR #{} now that all checks pass.", pr_number));
            }
            PrState::HasErrors { pr_number, errors } => {
                self.messages.push(AppMessage::User(format!(
                    "Fix errors in PR #{} and commit.",
                    pr_number
                )));
                self.is_processing = true;
                self.status_bar.left = "PROCESSING".into();
                self.status_bar.center = "Fixing PR errors...".into();
                let _ = self.agent_tx.try_send(format!(
                    "PR #{} has errors: {}. Fix the issues and commit the changes.",
                    pr_number, errors
                ));
            }
            PrState::None => {}
        }
    }

    pub fn submit_input(&mut self) {
        let input = self.input_buffer.trim().to_string();
        if input.is_empty() && self.attachments.is_empty() {
            return;
        }

        self.input_buffer.clear();
        self.cursor_pos = 0;
        self.show_commands = false;

        if input.starts_with('/') {
            self.handle_command(&input);
            return;
        }

        let mut full_message = String::new();

        if !self.attachments.is_empty() {
            for att in &self.attachments {
                if let Some(ref content) = att.content {
                    full_message.push_str(&format!(
                        "--- {} ({}) ---\n{}\n",
                        att.kind.icon(),
                        att.path,
                        content
                    ));
                } else {
                    full_message.push_str(&format!(
                        "--- {} {} (binary, {} bytes) ---\n",
                        att.kind.icon(),
                        att.path,
                        std::fs::metadata(&att.path).map(|m| m.len()).unwrap_or(0)
                    ));
                }
            }
            full_message.push_str("\n--- User Message ---\n");
        }

        full_message.push_str(&input);

        let display_parts: Vec<&str> = input.lines().take(3).collect();
        let preview = if input.lines().count() > 3 {
            format!("{}...", display_parts.join("\n"))
        } else {
            input.clone()
        };

        if !self.attachments.is_empty() {
            let att_summary: Vec<String> = self
                .attachments
                .iter()
                .map(|a| format!("{} {}", a.kind.icon(), a.path))
                .collect();
            self.messages.push(AppMessage::User(format!(
                "{}\n[Attachments: {}]",
                preview,
                att_summary.join(", ")
            )));
        } else {
            self.messages.push(AppMessage::User(input.clone()));
        }

        self.attachments.clear();
        self.is_processing = true;
        self.status_bar.left = "PROCESSING".into();
        self.status_bar.center = "Working...".into();
        self.update_conversation_context();

        let _ = self.agent_tx.try_send(full_message);
    }

    fn handle_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let name = parts[0].trim_start_matches('/');

        match name {
            "help" => {
                self.messages
                    .push(AppMessage::System("Available commands:".into()));
                for c in COMMANDS {
                    self.messages.push(AppMessage::System(format!(
                        "  {:<12} — {}",
                        c.usage, c.description
                    )));
                }
            }
            "clear" => {
                self.messages.clear();
                self.messages
                    .push(AppMessage::System("Chat cleared".into()));
            }
            "model" => {
                if let Some(model) = parts.get(1) {
                    self.current_model = model.trim().to_string();
                    self.messages.push(AppMessage::System(format!(
                        "Model set to: {}",
                        model.trim()
                    )));
                } else {
                    self.messages.push(AppMessage::System(format!(
                        "Current model: {}",
                        self.current_model
                    )));
                }
            }
            "provider" => {
                if let Some(provider) = parts.get(1) {
                    self.messages.push(AppMessage::System(format!(
                        "Provider set to: {}",
                        provider.trim()
                    )));
                } else {
                    self.messages
                        .push(AppMessage::System("Current provider: check config".into()));
                }
            }
            "mode" => {
                if let Some(mode) = parts.get(1) {
                    self.messages
                        .push(AppMessage::System(format!("Mode set to: {}", mode.trim())));
                } else {
                    self.messages
                        .push(AppMessage::System("Current mode: build".into()));
                }
            }
            "compact" => {
                self.messages
                    .push(AppMessage::System("Context compacted".into()));
            }
            "doctor" => {
                self.messages.push(AppMessage::System(
                    "Configuration check — run `oni doctor` for details".into(),
                ));
            }
            "setup" => {
                self.messages.push(AppMessage::System(
                    "Run `oni setup` from the command line to re-run the wizard".into(),
                ));
            }
            "exit" | "quit" => {
                self.active = false;
            }
            "attach" | "file" => {
                if let Some(path) = parts.get(1) {
                    let path = path.trim().to_string();
                    if std::path::Path::new(&path).exists() {
                        let kind = AttachmentKind::from_path(&path);
                        let content = if matches!(kind, AttachmentKind::Text) {
                            std::fs::read_to_string(&path).ok()
                        } else {
                            None
                        };
                        let filename = std::path::Path::new(&path)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.clone());
                        let kind_icon = kind.icon();
                        let filename_clone = filename.clone();
                        self.attachments.push(Attachment {
                            path: filename,
                            kind,
                            content,
                        });
                        self.messages.push(AppMessage::System(format!(
                            "Attached: {} {}",
                            kind_icon, filename_clone
                        )));
                    } else {
                        self.messages
                            .push(AppMessage::Error(format!("File not found: {}", path)));
                    }
                } else {
                    self.file_picker_active = true;
                    self.file_picker_path = self.working_directory.clone();
                    self.refresh_file_picker();
                }
            }
            "attachments" | "files" => {
                if self.attachments.is_empty() {
                    self.messages
                        .push(AppMessage::System("No attachments".into()));
                } else {
                    for att in &self.attachments {
                        self.messages.push(AppMessage::System(format!(
                            "  {} {} ({})",
                            att.kind.icon(),
                            att.path,
                            att.content
                                .as_ref()
                                .map(|c| format!("{} bytes", c.len()))
                                .unwrap_or_else(|| "binary".into())
                        )));
                    }
                }
            }
            "clear-attachments" => {
                self.attachments.clear();
                self.messages
                    .push(AppMessage::System("Attachments cleared".into()));
            }
            unknown => {
                self.messages.push(AppMessage::Error(format!(
                    "Unknown command: /{unknown}. Type /help for available commands."
                )));
            }
        }
    }

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

        if key.kind != KeyEventKind::Press {
            return;
        }

        if !matches!(self.question_mode, QuestionMode::None) {
            self.handle_question_key(key);
            return;
        }

        if self.file_picker_active {
            self.handle_file_picker_key(key);
            return;
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.active = false;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('l')) => {
                self.messages.clear();
                self.messages
                    .push(AppMessage::System("Chat cleared".into()));
            }
            (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.input_buffer.clear();
                self.cursor_pos = 0;
                self.show_commands = false;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                self.show_commands = !self.show_commands;
                if self.show_commands {
                    self.command_filter = String::new();
                }
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                if self.show_commands {
                    self.show_commands = false;
                    self.command_filter.clear();
                } else if self.is_processing {
                    self.active = false;
                }
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                if self.show_commands {
                    let matches: Vec<&CommandDef> = COMMANDS
                        .iter()
                        .filter(|c| c.name.starts_with(&self.command_filter))
                        .collect();
                    if matches.len() == 1 {
                        self.input_buffer = format!("/{} ", matches[0].name);
                        self.cursor_pos = self.input_buffer.len();
                        self.show_commands = false;
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                self.scroll_offset += 1;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.submit_input();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if self.show_commands && !self.command_filter.is_empty() {
                    self.command_filter.pop();
                } else if self.cursor_pos > 0 {
                    self.input_buffer.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                }
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                self.input_buffer.insert(self.cursor_pos, c);
                self.cursor_pos += 1;

                if self.input_buffer.starts_with('/') {
                    self.show_commands = true;
                    self.command_filter = self.input_buffer[1..].to_string();
                } else {
                    self.show_commands = false;
                    self.command_filter.clear();
                }
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                if self.cursor_pos < self.input_buffer.len() {
                    self.cursor_pos += 1;
                }
            }
            _ => {}
        }
    }

    fn refresh_file_picker(&mut self) {
        self.file_picker_entries.clear();
        self.file_picker_selected = 0;
        self.file_picker_offset = 0;

        if let Ok(entries) = std::fs::read_dir(&self.file_picker_path) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();

            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with('.') {
                        continue;
                    }
                    let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
                    if is_dir {
                        dirs.push(format!("{name}{}", std::path::MAIN_SEPARATOR));
                    } else {
                        files.push(name.to_string());
                    }
                }
            }

            dirs.sort();
            files.sort();

            self.file_picker_entries.push("..".to_string());
            self.file_picker_entries.extend(dirs);
            self.file_picker_entries.extend(files);
        }
    }

    fn handle_file_picker_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.file_picker_active = false;
            }
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.file_picker_active = false;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(entry) = self.file_picker_entries.get(self.file_picker_selected) {
                    if entry == ".." {
                        if let Some(parent) = std::path::Path::new(&self.file_picker_path).parent()
                        {
                            self.file_picker_path = parent.to_string_lossy().to_string();
                            self.refresh_file_picker();
                        }
                    } else {
                        let full_path =
                            format!("{}{}{}", self.file_picker_path, std::path::MAIN_SEPARATOR, entry.trim_end_matches(std::path::MAIN_SEPARATOR));
                        if entry.ends_with(std::path::MAIN_SEPARATOR) {
                            self.file_picker_path = full_path;
                            self.refresh_file_picker();
                        } else {
                            let kind = AttachmentKind::from_path(entry);
                            let content = if matches!(kind, AttachmentKind::Text) {
                                std::fs::read_to_string(&full_path).ok()
                            } else {
                                None
                            };
                            let kind_icon = kind.icon();
                            let entry_clone = entry.clone();
                            self.attachments.push(Attachment {
                                path: entry.clone(),
                                kind,
                                content,
                            });
                            self.messages.push(AppMessage::System(format!(
                                "Attached: {} {}",
                                kind_icon, entry_clone
                            )));
                            self.file_picker_active = false;
                        }
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                if self.file_picker_selected > 0 {
                    self.file_picker_selected -= 1;
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                if self.file_picker_selected + 1 < self.file_picker_entries.len() {
                    self.file_picker_selected += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_question_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.answer_question_with_selection();
            }
            (KeyModifiers::NONE, KeyCode::Char('t')) => {
                self.question_mode = QuestionMode::Typing;
                self.messages
                    .push(AppMessage::System("Type your answer:".into()));
            }
            (KeyModifiers::NONE, KeyCode::Char('0'..='9')) => {
                if let QuestionMode::Selecting(_) = self.question_mode {
                    if let Some((ref req, _)) = self.pending_question {
                        if let KeyCode::Char(c) = key.code {
                            let idx = c.to_digit(10).unwrap_or(0) as usize;
                            if idx < req.options.len() {
                                self.question_selected = idx;
                                self.messages.push(AppMessage::System(format!(
                                    "  Selected: {}",
                                    req.options[idx].label
                                )));
                            }
                        }
                    }
                }
            }
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                self.pending_question = None;
                self.question_mode = QuestionMode::None;
                self.is_processing = false;
                self.status_bar.left = "NORMAL".into();
                self.status_bar.center = "Question cancelled".into();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                if matches!(self.question_mode, QuestionMode::Typing) {
                    self.question_custom_input.pop();
                }
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                if matches!(self.question_mode, QuestionMode::Typing) {
                    self.question_custom_input.push(c);
                }
            }
            _ => {}
        }
    }

    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking(text) => {
                self.messages.push(AppMessage::Thinking(text));
                self.scroll_offset = 0;
            }
            AgentEvent::ToolCall { name, input } => {
                let task_title = format!("{} {}", name, input.chars().take(30).collect::<String>());
                if !self.tasks.iter().any(|t| t.title == task_title) {
                    self.tasks.push(Task {
                        title: task_title,
                        status: TaskStatus::InProgress,
                    });
                }

                self.messages.push(AppMessage::ToolCall {
                    name: name.clone(),
                    input,
                });
                self.status_bar.center = format!("Running: {name}");
                self.scroll_offset = 0;
            }
            AgentEvent::ToolResult {
                name,
                output: _,
                is_error,
            } => {
                if let Some(task) = self
                    .tasks
                    .iter_mut()
                    .rev()
                    .find(|t| t.title.starts_with(&name) && t.status == TaskStatus::InProgress)
                {
                    task.status = if is_error {
                        TaskStatus::Pending
                    } else {
                        TaskStatus::Complete
                    };
                }

                self.messages
                    .push(AppMessage::ToolResult { name, is_error });
            }
            AgentEvent::AssistantMessage(text) => {
                self.streaming_buffer.push_str(&text);
                self.update_conversation_context();
            }
            AgentEvent::TurnComplete => {
                if !self.streaming_buffer.is_empty() {
                    self.messages
                        .push(AppMessage::Assistant(self.streaming_buffer.clone()));
                    self.streaming_buffer.clear();
                }
                self.is_processing = false;
                self.status_bar.left = "NORMAL".into();
                self.status_bar.center = "".into();
            }
            AgentEvent::TokenUsage { input, output } => {
                self.context_used = input + output;
                self.context_breakdown.conversation = (input + output) as usize;
                self.status_bar.right = format!("tokens: {}↑ {}↓", input, output);
            }
            AgentEvent::Error(text) => {
                self.messages.push(AppMessage::Error(text));
                self.is_processing = false;
                self.status_bar.left = "ERROR".into();
            }
            AgentEvent::LoopDetected { tool, count } => {
                self.messages.push(AppMessage::Error(format!(
                    "Loop detected: {tool} called {count} times with same input"
                )));
                self.is_processing = false;
                self.status_bar.left = "LOOP".into();
            }
            AgentEvent::PermissionRequest {
                tool,
                input,
                reason,
            } => {
                self.messages.push(AppMessage::System(format!(
                    "Permission requested: {tool} — {input}"
                )));
                if !reason.is_empty() {
                    self.messages
                        .push(AppMessage::System(format!("  Reason: {reason}")));
                }
                self.status_bar.center = format!("Waiting approval: {tool}");
            }
            AgentEvent::QuestionAsked { question, header } => {
                self.messages
                    .push(AppMessage::System(format!("[{header}] {question}")));
                self.status_bar.center = "Question pending".into();
            }
            AgentEvent::SubAgentStarted { id, name, task } => {
                self.subagents.push(SubAgentCard {
                    id,
                    name,
                    task,
                    status: SubAgentStatus::Thinking,
                    output: String::new(),
                });
                self.scroll_offset = 0;
            }
            AgentEvent::SubAgentOutput { id, name: _, text } => {
                if let Some(card) = self.subagents.iter_mut().find(|c| c.id == id) {
                    card.output.push_str(&text);
                    card.status = SubAgentStatus::Working;
                }
            }
            AgentEvent::SubAgentComplete {
                id,
                name: _,
                result,
            } => {
                if let Some(card) = self.subagents.iter_mut().find(|c| c.id == id) {
                    card.status = SubAgentStatus::Complete;
                    if !result.is_empty() {
                        card.output = result;
                    }
                }
            }
            AgentEvent::MessageQueued { priority } => {
                self.messages
                    .push(AppMessage::System(format!("Message queued: {priority}")));
            }
            AgentEvent::CompactionStarted => {
                self.messages
                    .push(AppMessage::System("Compacting context...".into()));
                self.status_bar.center = "Compacting...".into();
            }
            AgentEvent::CompactionComplete { summary } => {
                self.messages
                    .push(AppMessage::System(format!("Context compacted: {summary}")));
                self.status_bar.center = "".into();
            }
            AgentEvent::MaxStepsApproaching { current, max } => {
                self.messages.push(AppMessage::System(format!(
                    "Max steps reached ({current}/{max}), asking model to wrap up..."
                )));
            }
        }
    }
        }
    }

    fn init_terminal(&self) -> anyhow::Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
        use std::io::stdout;

        use crossterm::{
            execute,
            terminal::{EnterAlternateScreen, enable_raw_mode},
        };

        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(terminal)
    }

    fn cleanup_terminal(&self) -> anyhow::Result<()> {
        use std::io::stdout;

        use crossterm::{
            execute,
            terminal::{LeaveAlternateScreen, disable_raw_mode},
        };

        let mut stdout = stdout();
        execute!(stdout, LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }
}
