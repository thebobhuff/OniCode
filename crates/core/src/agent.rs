use std::{collections::VecDeque, sync::Arc};

use futures::StreamExt;
use onicode_config::{Hook, HookContext, HookEvent};
use tokio::sync::{mpsc, watch};
use tracing::{debug, error, info, warn};

use crate::{
    client::LlmClient,
    error::{CoreError, Result},
    message::{Message, MessagePart, ToolPartState},
    message_queue::{MessagePriority, MessageQueue, QueueMode},
    permissions::{PendingPermissionReason, PermissionDecision, PermissionManager},
    question::QuestionBridge,
    session::Session,
    tool::ToolRegistry,
};

const DOOM_LOOP_THRESHOLD: usize = 3;
const MAX_STEPS_PROMPT: &str = "You have reached the maximum number of steps. Wrap up your response now and provide a summary of what was accomplished. Do NOT call any more tools.";

#[derive(Debug, Clone)]
pub enum AgentEvent {
    Thinking(String),
    ToolCall {
        name: String,
        input: String,
    },
    ToolResult {
        name: String,
        output: String,
        is_error: bool,
    },
    AssistantMessage(String),
    TokenUsage {
        input: u64,
        output: u64,
    },
    TurnComplete,
    Error(String),
    LoopDetected {
        tool: String,
        count: usize,
    },
    PermissionRequest {
        tool: String,
        input: String,
        reason: String,
    },
    QuestionAsked {
        question: String,
        header: String,
    },
    SubAgentStarted {
        id: usize,
        name: String,
        task: String,
    },
    SubAgentComplete {
        id: usize,
        name: String,
        result: String,
    },
    SubAgentOutput {
        id: usize,
        name: String,
        text: String,
    },
    MessageQueued {
        priority: String,
    },
    CompactionStarted,
    CompactionComplete {
        summary: String,
    },
    MaxStepsApproaching {
        current: usize,
        max: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentMode {
    Build,
    Plan,
}

impl AgentMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "plan" => AgentMode::Plan,
            _ => AgentMode::Build,
        }
    }

    pub fn is_read_only(&self) -> bool {
        matches!(self, AgentMode::Plan)
    }

    pub fn blocked_tools(&self) -> &'static [&'static str] {
        match self {
            AgentMode::Plan => &["write", "edit", "bash"],
            AgentMode::Build => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RunnerState {
    Idle,
    Busy,
    WaitingPermission,
    WaitingQuestion,
}

pub struct Agent {
    client: Arc<LlmClient>,
    tools: Arc<ToolRegistry>,
    max_turns: usize,
    event_tx: mpsc::Sender<AgentEvent>,
    loop_window: usize,
    loop_max_repeats: usize,
    permissions: Arc<PermissionManager>,
    mode: AgentMode,
    question_bridge: QuestionBridge,
    sub_agents: Arc<Vec<SubAgent>>,
    queue: MessageQueue,
    auto_compact: bool,
    compaction_threshold: usize,
    compaction_keep: usize,
    max_parallel_subagents: usize,
    active_subagents: Arc<std::sync::atomic::AtomicUsize>,
    hooks: Vec<Hook>,
    session_id: String,
    state_tx: watch::Sender<RunnerState>,
    state_rx: watch::Receiver<RunnerState>,
}

#[derive(Debug, Clone)]
pub struct SubAgent {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub max_turns: usize,
    pub read_only: bool,
}

impl SubAgent {
    pub fn explore() -> Self {
        Self {
            name: "explore".to_string(),
            description: "Fast, read-only agent for codebase exploration".to_string(),
            system_prompt: "You are a read-only codebase exploration agent. Search and read files to understand the codebase structure. Do NOT write, edit, or execute anything.".to_string(),
            tools: vec!["read".to_string(), "grep".to_string(), "glob".to_string(), "ls".to_string()],
            max_turns: 10,
            read_only: true,
        }
    }

    pub fn general() -> Self {
        Self {
            name: "general".to_string(),
            description: "General-purpose agent for research and multi-step tasks".to_string(),
            system_prompt: "You are a general-purpose agent. You can read files, search code, and execute commands to complete tasks.".to_string(),
            tools: vec!["read".to_string(), "write".to_string(), "edit".to_string(), "bash".to_string(), "grep".to_string(), "glob".to_string(), "ls".to_string()],
            max_turns: 20,
            read_only: false,
        }
    }
}

impl Agent {
    pub fn new(
        client: Arc<LlmClient>,
        tools: Arc<ToolRegistry>,
        max_turns: usize,
        event_tx: mpsc::Sender<AgentEvent>,
    ) -> Self {
        let (state_tx, state_rx) = watch::channel(RunnerState::Idle);
        Self {
            client,
            tools,
            max_turns,
            event_tx,
            loop_window: 8,
            loop_max_repeats: DOOM_LOOP_THRESHOLD,
            permissions: Arc::new(PermissionManager::new(onicode_config::PermissionMode::Ask)),
            mode: AgentMode::Build,
            question_bridge: QuestionBridge::new(),
            sub_agents: Arc::new(vec![SubAgent::explore(), SubAgent::general()]),
            queue: MessageQueue::new(QueueMode::All),
            auto_compact: true,
            compaction_threshold: 80,
            compaction_keep: 10,
            max_parallel_subagents: 5,
            active_subagents: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            hooks: Vec::new(),
            session_id: String::new(),
            state_tx,
            state_rx,
        }
    }

    pub fn state(&self) -> RunnerState {
        self.state_rx.borrow().clone()
    }

    pub fn state_rx(&self) -> watch::Receiver<RunnerState> {
        self.state_rx.clone()
    }

    pub fn with_loop_detection(mut self, window: usize, max_repeats: usize) -> Self {
        self.loop_window = window;
        self.loop_max_repeats = max_repeats;
        self
    }

    pub fn with_permissions(mut self, permissions: Arc<PermissionManager>) -> Self {
        self.permissions = permissions;
        self
    }

    pub fn with_mode(mut self, mode: AgentMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_question_bridge(mut self, bridge: QuestionBridge) -> Self {
        self.question_bridge = bridge;
        self
    }

    pub fn with_sub_agents(mut self, agents: Vec<SubAgent>) -> Self {
        self.sub_agents = Arc::new(agents);
        self
    }

    pub fn with_hooks(mut self, hooks: Vec<Hook>, session_id: String) -> Self {
        self.hooks = hooks;
        self.session_id = session_id;
        self
    }

    pub fn with_message_queue(mut self, queue: MessageQueue) -> Self {
        self.queue = queue;
        self
    }

    pub fn with_auto_compact(mut self, threshold: usize, keep: usize) -> Self {
        self.auto_compact = true;
        self.compaction_threshold = threshold;
        self.compaction_keep = keep;
        self
    }

    pub fn queue_steering(&self, message: Message) {
        self.queue.enqueue(message, MessagePriority::Steering);
        let _ = self.event_tx.try_send(AgentEvent::MessageQueued {
            priority: "steering".to_string(),
        });
    }

    pub fn queue_followup(&self, message: Message) {
        self.queue.enqueue(message, MessagePriority::FollowUp);
        let _ = self.event_tx.try_send(AgentEvent::MessageQueued {
            priority: "followup".to_string(),
        });
    }

    pub async fn run(&self, session: &mut Session, user_input: &str) -> Result<()> {
        info!(session_id = %session.id, user_input, "Starting agent turn");

        session.add_message(Message::user(user_input));

        self.run_hooks(
            HookEvent::SessionStart,
            HookContext {
                session_id: session.id.to_string(),
                message: user_input.to_string(),
                ..Default::default()
            },
        )
        .await;

        self.run_hooks(
            HookEvent::UserPrompt,
            HookContext {
                session_id: session.id.to_string(),
                message: user_input.to_string(),
                ..Default::default()
            },
        )
        .await;

        self.outer_loop(session).await
    }

    async fn set_state(&self, state: RunnerState) {
        let _ = self.state_tx.send(state);
    }

    async fn outer_loop(&self, session: &mut Session) -> Result<()> {
        loop {
            self.set_state(RunnerState::Busy).await;
            let has_more = self.inner_loop(session).await?;

            self.set_state(RunnerState::Idle).await;

            let followups = self.queue.drain_followup();
            if followups.is_empty() || !has_more {
                break;
            }

            for msg in followups {
                session.add_message(msg);
            }
        }

        self.run_hooks(
            HookEvent::SessionEnd,
            HookContext {
                session_id: session.id.to_string(),
                ..Default::default()
            },
        )
        .await;

        info!(
            session_id = %session.id,
            tokens = session.token_count(),
            "Agent session complete"
        );

        Ok(())
    }

    async fn inner_loop(&self, session: &mut Session) -> Result<bool> {
        let mut turn_count = 0;
        let mut tool_call_history: VecDeque<(String, String)> =
            VecDeque::with_capacity(self.loop_window);

        if self.mode.is_read_only() {
            session.add_message(Message::system(
                "You are in PLAN MODE. You can read files and search code, but you cannot write, edit, or execute commands. Analyze and propose changes without making them.",
            ));
        }

        loop {
            let is_last_step = turn_count >= self.max_turns;

            if is_last_step {
                warn!(
                    "Max turns ({}) reached, injecting wrap-up prompt",
                    self.max_turns
                );
                self.emit(AgentEvent::MaxStepsApproaching {
                    current: turn_count,
                    max: self.max_turns,
                })
                .await;
                session.add_message(Message::system(MAX_STEPS_PROMPT));
            }

            let steering = self.queue.drain_steering();
            for msg in steering {
                session.add_message(msg);
            }

            if self.auto_compact
                && crate::compaction::should_compact(session, self.compaction_threshold)
            {
                self.set_state(RunnerState::Busy).await;
                self.emit(AgentEvent::CompactionStarted).await;
                if let Some(summary) =
                    crate::compaction::compact_session(session, self.compaction_keep)
                {
                    debug!(summary = %summary.content, "Session compacted");
                    self.emit(AgentEvent::CompactionComplete {
                        summary: format!(
                            "{} file operations tracked",
                            summary.file_operations.len()
                        ),
                    })
                    .await;
                }
            }

            turn_count += 1;

            let all_tools = self.tools.list();
            let available_tools: Vec<_> = all_tools
                .into_iter()
                .filter(|t| {
                    !self.mode.blocked_tools().contains(&t.name.as_str())
                        && !self.permissions.is_tool_denied(&t.name)
                })
                .collect();

            let messages = &session.messages;

            self.emit(AgentEvent::Thinking("Waiting for response...".to_string()))
                .await;

            let mut stream = match self.client.chat_stream(messages, &available_tools) {
                Ok(s) => s,
                Err(e) => {
                    error!(error = %e, "LLM stream creation failed");
                    self.emit(AgentEvent::Error(format!("LLM error: {e}")))
                        .await;
                    return Ok(false);
                }
            };

            let mut content = String::new();
            let mut thinking = String::new();
            let mut tool_calls = Vec::new();
            let mut stop_reason = "unknown".to_string();
            let mut usage = crate::client::TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
            };

            while let Some(event) = stream.next().await {
                match event {
                    Ok(crate::client::StreamEvent::Text(text)) => {
                        content.push_str(&text);
                        self.emit(AgentEvent::AssistantMessage(text)).await;
                    }
                    Ok(crate::client::StreamEvent::Thinking(text)) => {
                        thinking.push_str(&text);
                        self.emit(AgentEvent::Thinking(text)).await;
                    }
                    Ok(crate::client::StreamEvent::ToolCall(tc)) => {
                        tool_calls.push(tc.clone());
                        self.emit(AgentEvent::ToolCall {
                            name: tc.name.clone(),
                            input: tc.input.to_string(),
                        })
                        .await;
                    }
                    Ok(crate::client::StreamEvent::Done {
                        stop_reason: sr,
                        usage: u,
                    }) => {
                        stop_reason = sr;
                        usage = u;
                    }
                    Err(e) => {
                        error!(error = %e, "Stream error");
                        self.emit(AgentEvent::Error(format!("Stream error: {e}")))
                            .await;
                        return Ok(false);
                    }
                }
            }

            session.metadata.total_input_tokens += usage.input_tokens;
            session.metadata.total_output_tokens += usage.output_tokens;

            self.emit(AgentEvent::TokenUsage {
                input: usage.input_tokens,
                output: usage.output_tokens,
            })
            .await;

            if !content.is_empty() {
                debug!(content = %content, "Assistant response");
                session.add_message(Message::assistant(&content));

                self.run_hooks(
                    HookEvent::AgentResponse,
                    HookContext {
                        session_id: session.id.to_string(),
                        message: content.clone(),
                        ..Default::default()
                    },
                )
                .await;
            }

            if tool_calls.is_empty() {
                debug!("No tool calls, turn complete");
                self.emit(AgentEvent::TurnComplete).await;
                return Ok(self.queue.has_pending());
            }

            for tool_call in &tool_calls {
                let key = (tool_call.name.clone(), tool_call.input.to_string());
                tool_call_history.push_back(key.clone());
                if tool_call_history.len() > self.loop_window {
                    tool_call_history.pop_front();
                }

                let repeats = tool_call_history.iter().filter(|k| *k == &key).count();

                if repeats >= self.loop_max_repeats {
                    warn!(
                        tool = %tool_call.name,
                        repeats = repeats,
                        "Doom loop detected: tool called {} times with same input",
                        repeats
                    );

                    self.emit(AgentEvent::LoopDetected {
                        tool: tool_call.name.clone(),
                        count: repeats,
                    })
                    .await;

                    self.emit(AgentEvent::PermissionRequest {
                        tool: tool_call.name.clone(),
                        input: format!(
                            "Same tool+input called {repeats} times. Continue? (doom_loop protection)"
                        ),
                        reason: format!("Doom loop: {tool_call.name} called {repeats} times with identical input"),
                    })
                    .await;

                    self.set_state(RunnerState::WaitingPermission).await;

                    let decision_rx = self.permissions.request_pending_permission(
                        tool_call.name.clone(),
                        tool_call.input.to_string(),
                        PendingPermissionReason::DoomLoop { count: repeats },
                    );

                    let decision = self.wait_for_permission(decision_rx).await;

                    match decision {
                        PermissionDecision::Allow => {
                            info!(tool = %tool_call.name, "Doom loop override: user allowed continuation");
                        }
                        PermissionDecision::Deny | PermissionDecision::Pending => {
                            info!(tool = %tool_call.name, "Doom loop: user denied or cancelled");
                            session.add_message(Message::tool_result(
                                tool_call.id.clone(),
                                "Stopped by doom loop protection — user denied continuation"
                                    .to_string(),
                                true,
                            ));
                            self.emit(AgentEvent::ToolResult {
                                name: tool_call.name.clone(),
                                output: "Stopped by doom loop protection".to_string(),
                                is_error: true,
                            })
                            .await;
                            self.emit(AgentEvent::TurnComplete).await;
                            return Ok(false);
                        }
                    }
                }

                if self.mode.blocked_tools().contains(&tool_call.name.as_str()) {
                    let msg = format!(
                        "Tool '{}' is blocked in {} mode",
                        tool_call.name,
                        match self.mode {
                            AgentMode::Plan => "plan",
                            AgentMode::Build => "build",
                        }
                    );
                    session.add_message(Message::tool_result(
                        tool_call.id.clone(),
                        msg.clone(),
                        true,
                    ));
                    self.emit(AgentEvent::ToolResult {
                        name: tool_call.name.clone(),
                        output: msg,
                        is_error: true,
                    })
                    .await;
                    continue;
                }

                if tool_call.name == "task" {
                    self.handle_task_tool(session, tool_call).await;
                    continue;
                }

                if tool_call.name == "question" {
                    self.handle_question_tool(session, tool_call).await;
                    continue;
                }

                if self.permissions.needs_approval(&tool_call.name) {
                    self.emit(AgentEvent::PermissionRequest {
                        tool: tool_call.name.clone(),
                        input: tool_call.input.to_string(),
                        reason: format!("Tool '{}' requires approval", tool_call.name),
                    })
                    .await;

                    self.set_state(RunnerState::WaitingPermission).await;

                    let decision_rx = self.permissions.request_pending_permission(
                        tool_call.name.clone(),
                        tool_call.input.to_string(),
                        PendingPermissionReason::Normal,
                    );

                    let decision = self.wait_for_permission(decision_rx).await;

                    match decision {
                        PermissionDecision::Allow => {}
                        PermissionDecision::Deny => {
                            session.add_message(Message::tool_result(
                                tool_call.id.clone(),
                                "Permission denied by user".to_string(),
                                true,
                            ));
                            self.emit(AgentEvent::ToolResult {
                                name: tool_call.name.clone(),
                                output: "Permission denied".to_string(),
                                is_error: true,
                            })
                            .await;
                            continue;
                        }
                        PermissionDecision::Pending => {
                            session.add_message(Message::tool_result(
                                tool_call.id.clone(),
                                "Permission request cancelled".to_string(),
                                true,
                            ));
                            self.emit(AgentEvent::ToolResult {
                                name: tool_call.name.clone(),
                                output: "Permission cancelled".to_string(),
                                is_error: true,
                            })
                            .await;
                            continue;
                        }
                    }
                }
            }

            session.add_message(Message::assistant_with_tool_calls(
                &content,
                tool_calls.clone(),
            ));

            for tool_call in &tool_calls {
                if self.mode.blocked_tools().contains(&tool_call.name.as_str()) {
                    continue;
                }

                if tool_call.name == "task" || tool_call.name == "question" {
                    continue;
                }

                if self.permissions.is_tool_denied(&tool_call.name) {
                    continue;
                }

                debug!(
                    tool = %tool_call.name,
                    input = %tool_call.input,
                    "Executing tool"
                );

                self.run_hooks(
                    HookEvent::PreToolUse,
                    HookContext {
                        session_id: session.id.to_string(),
                        tool_name: tool_call.name.clone(),
                        command: tool_call.input.to_string(),
                        ..Default::default()
                    },
                )
                .await;

                self.emit(AgentEvent::ToolCall {
                    name: tool_call.name.clone(),
                    input: tool_call.input.to_string(),
                })
                .await;

                let input = crate::tool::ToolInput {
                    parameters: if tool_call.input.is_object() {
                        tool_call
                            .input
                            .as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    } else {
                        Default::default()
                    },
                    session_id: Some(session.id.to_string()),
                };

                let input_str = tool_call.input.to_string();
                let tool_name = tool_call.name.clone();

                let result = match self.tools.execute(&tool_call.name, input).await {
                    Ok(output) => output,
                    Err(e) => {
                        error!(tool = %tool_call.name, error = %e, "Tool execution failed");
                        self.run_hooks(
                            HookEvent::ToolError,
                            HookContext {
                                session_id: session.id.to_string(),
                                tool_name: tool_name.clone(),
                                command: input_str.clone(),
                                message: e.to_string(),
                                ..Default::default()
                            },
                        )
                        .await;
                        crate::tool::ToolOutput::error(format!("Tool execution failed: {e}"))
                    }
                };

                self.run_hooks(
                    HookEvent::PostToolUse,
                    HookContext {
                        session_id: session.id.to_string(),
                        tool_name: tool_name.clone(),
                        command: input_str.clone(),
                        output: result.content.clone(),
                        ..Default::default()
                    },
                )
                .await;

                session.metadata.total_tool_calls += 1;

                self.emit(AgentEvent::ToolResult {
                    name: tool_call.name.clone(),
                    output: result.content.clone(),
                    is_error: result.is_error,
                })
                .await;

                session.add_message(Message::tool_result(
                    tool_call.id.clone(),
                    result.content,
                    result.is_error,
                ));
            }

            if stop_reason == "end_turn" || stop_reason == "stop" {
                self.emit(AgentEvent::TurnComplete).await;
                return Ok(self.queue.has_pending());
            }

            if is_last_step {
                self.emit(AgentEvent::TurnComplete).await;
                return Ok(false);
            }
        }
    }

    async fn wait_for_permission(
        &self,
        mut decision_rx: watch::Receiver<PermissionDecision>,
    ) -> PermissionDecision {
        loop {
            tokio::select! {
                Ok(()) = decision_rx.changed() => {
                    let decision = decision_rx.borrow().clone();
                    if decision != PermissionDecision::Pending {
                        self.set_state(RunnerState::Busy).await;
                        return decision;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    self.set_state(RunnerState::Busy).await;
                    return PermissionDecision::Pending;
                }
            }
        }
    }

    async fn handle_task_tool(&self, session: &mut Session, tool_call: &crate::message::ToolCall) {
        let agent_name = tool_call.input["agent"].as_str().unwrap_or("general");
        let task = tool_call.input["task"].as_str().unwrap_or("");
        let description = tool_call.input["description"].as_str().unwrap_or(task);

        if task.is_empty() {
            session.add_message(Message::tool_result(
                tool_call.id.clone(),
                "Error: 'task' parameter is required".to_string(),
                true,
            ));
            self.emit(AgentEvent::ToolResult {
                name: "task".to_string(),
                output: "Missing task parameter".to_string(),
                is_error: true,
            })
            .await;
            return;
        }

        let sub_agent = self.sub_agents.iter().find(|a| a.name == agent_name);

        let sub_agent = match sub_agent {
            Some(a) => a,
            None => {
                let available: Vec<&str> =
                    self.sub_agents.iter().map(|a| a.name.as_str()).collect();
                session.add_message(Message::tool_result(
                    tool_call.id.clone(),
                    format!(
                        "Error: Unknown subagent '{agent_name}'. Available: {}",
                        available.join(", ")
                    ),
                    true,
                ));
                self.emit(AgentEvent::ToolResult {
                    name: "task".to_string(),
                    output: format!("Unknown subagent: {agent_name}"),
                    is_error: true,
                })
                .await;
                return;
            }
        };

        let sub_agent_id = self
            .active_subagents
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if sub_agent_id >= self.max_parallel_subagents {
            self.active_subagents
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            session.add_message(Message::tool_result(
                tool_call.id.clone(),
                format!(
                    "Error: Maximum parallel subagents ({}) reached",
                    self.max_parallel_subagents
                ),
                true,
            ));
            self.emit(AgentEvent::ToolResult {
                name: "task".to_string(),
                output: "Max subagents reached".to_string(),
                is_error: true,
            })
            .await;
            return;
        }

        self.emit(AgentEvent::SubAgentStarted {
            id: sub_agent_id,
            name: sub_agent.name.clone(),
            task: description.to_string(),
        })
        .await;

        info!(
            subagent_id = sub_agent_id,
            name = %sub_agent.name,
            task = %description,
            "Starting subagent"
        );

        let mut sub_session = Session::new(
            session.working_directory.clone(),
            self.client.config().model.clone(),
        );

        sub_session.add_message(Message::system(&sub_agent.system_prompt));
        sub_session.add_message(Message::user(task));

        let sub_tools: Vec<_> = self
            .tools
            .list()
            .into_iter()
            .filter(|t| sub_agent.tools.contains(&t.name) || !sub_agent.read_only)
            .collect();

        let sub_event_tx = self.event_tx.clone();
        let sub_client = self.client.clone();
        let sub_tools = Arc::new(sub_tools);
        let sub_agent_name = sub_agent.name.clone();
        let sub_max_turns = sub_agent.max_turns;
        let sub_task = task.to_string();

        let result = tokio::spawn(async move {
            let sub_agent_instance = Agent::new(sub_client, sub_tools, sub_max_turns, sub_event_tx)
                .with_permissions(Arc::new(PermissionManager::new(
                    onicode_config::PermissionMode::Auto,
                )))
                .with_mode(if sub_agent.read_only {
                    AgentMode::Plan
                } else {
                    AgentMode::Build
                })
                .with_loop_detection(8, 3);

            let mut turn_count = 0;
            let mut final_output = String::new();

            loop {
                if turn_count >= sub_max_turns {
                    break;
                }
                turn_count += 1;

                let available = sub_agent_instance.tools.list();
                let messages = &sub_session.messages;

                let mut stream = match sub_agent_instance.client.chat_stream(messages, &available) {
                    Ok(s) => s,
                    Err(e) => {
                        error!(error = %e, "Subagent LLM stream error");
                        break;
                    }
                };

                let mut content = String::new();
                let mut tool_calls = Vec::new();
                let mut stop_reason = "unknown".to_string();
                let mut usage = crate::client::TokenUsage::default();

                while let Some(event) = stream.next().await {
                    match event {
                        Ok(crate::client::StreamEvent::Text(text)) => {
                            content.push_str(&text);
                        }
                        Ok(crate::client::StreamEvent::ToolCall(tc)) => {
                            tool_calls.push(tc.clone());
                        }
                        Ok(crate::client::StreamEvent::Done {
                            stop_reason: sr,
                            usage: u,
                        }) => {
                            stop_reason = sr;
                            usage = u;
                        }
                        Err(_) => break,
                    }
                }

                sub_session.metadata.total_input_tokens += usage.input_tokens;
                sub_session.metadata.total_output_tokens += usage.output_tokens;

                if !content.is_empty() {
                    sub_session.add_message(Message::assistant(&content));
                    final_output = content.clone();
                }

                if tool_calls.is_empty() {
                    break;
                }

                sub_session.add_message(Message::assistant_with_tool_calls(
                    &content,
                    tool_calls.clone(),
                ));

                for tc in &tool_calls {
                    if sub_agent.read_only && ["write", "edit", "bash"].contains(&tc.name.as_str())
                    {
                        sub_session.add_message(Message::tool_result(
                            tc.id.clone(),
                            format!("Tool '{}' is blocked for read-only agent", tc.name),
                            true,
                        ));
                        continue;
                    }

                    let input = crate::tool::ToolInput {
                        parameters: if tc.input.is_object() {
                            tc.input
                                .as_object()
                                .unwrap()
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect()
                        } else {
                            Default::default()
                        },
                        session_id: Some(sub_session.id.to_string()),
                    };

                    let result = match sub_agent_instance.tools.execute(&tc.name, input).await {
                        Ok(output) => output,
                        Err(e) => {
                            crate::tool::ToolOutput::error(format!("Tool execution failed: {e}"))
                        }
                    };

                    sub_session.metadata.total_tool_calls += 1;

                    sub_session.add_message(Message::tool_result(
                        tc.id.clone(),
                        result.content.clone(),
                        result.is_error,
                    ));
                }

                if stop_reason == "end_turn" || stop_reason == "stop" {
                    break;
                }
            }

            final_output
        })
        .await
        .unwrap_or_default();

        self.active_subagents
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

        let result_wrapped = format!("<task_result>\n{result}\n</task_result>");

        session.add_message(Message::tool_result(
            tool_call.id.clone(),
            result_wrapped.clone(),
            false,
        ));

        self.emit(AgentEvent::SubAgentComplete {
            id: sub_agent_id,
            name: sub_agent_name,
            result: result.clone(),
        })
        .await;

        self.emit(AgentEvent::ToolResult {
            name: "task".to_string(),
            output: result_wrapped,
            is_error: false,
        })
        .await;

        info!(
            subagent_id = sub_agent_id,
            name = %sub_agent_name,
            "Subagent completed"
        );
    }

    async fn handle_question_tool(
        &self,
        session: &mut Session,
        tool_call: &crate::message::ToolCall,
    ) {
        let question = tool_call.input["question"].as_str().unwrap_or("");
        let header = tool_call.input["header"].as_str().unwrap_or("Question");

        self.emit(AgentEvent::QuestionAsked {
            question: question.to_string(),
            header: header.to_string(),
        })
        .await;

        if let Some(tx) = self.question_bridge.get_sender() {
            let options = tool_call.input["options"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|opt| {
                            Some(crate::question::QuestionOption {
                                label: opt["label"].as_str()?.to_string(),
                                description: opt["description"].as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            let multiple = tool_call.input["multiple"].as_bool().unwrap_or(false);

            let request = crate::question::QuestionRequest {
                question: question.to_string(),
                header: header.to_string(),
                options,
                multiple,
            };

            let (response_tx, response_rx) = tokio::sync::oneshot::channel();

            if tx.send((request, response_tx)).is_ok() {
                if let Ok(response) = response_rx.await {
                    let answers_json = serde_json::json!({
                        "answers": response.answers
                    });
                    session.add_message(Message::tool_result(
                        tool_call.id.clone(),
                        answers_json.to_string(),
                        false,
                    ));
                    self.emit(AgentEvent::ToolResult {
                        name: "question".to_string(),
                        output: answers_json.to_string(),
                        is_error: false,
                    })
                    .await;
                    return;
                }
            }
        }

        session.add_message(Message::tool_result(
            tool_call.id.clone(),
            "Question was cancelled".to_string(),
            true,
        ));
        self.emit(AgentEvent::ToolResult {
            name: "question".to_string(),
            output: "Question was cancelled".to_string(),
            is_error: true,
        })
        .await;
    }

    async fn emit(&self, event: AgentEvent) {
        if let Err(e) = self.event_tx.send(event).await {
            error!(error = %e, "Failed to send agent event");
        }
    }

    async fn run_hooks(&self, event: HookEvent, context: HookContext) {
        let matching_hooks: Vec<&Hook> = self.hooks.iter().filter(|h| h.event == event).collect();

        for hook in matching_hooks {
            match hook.execute(&context).await {
                Ok(output) => {
                    debug!(hook = ?event, output = %output, "Hook executed");
                }
                Err(err) => {
                    warn!(hook = ?event, error = %err, "Hook failed");
                }
            }
        }
    }
}
