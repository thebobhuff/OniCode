use std::{path::PathBuf, sync::Arc};

use clap::{CommandFactory, Parser, Subcommand};
use onicode_config::{OniCodeConfig, Settings};
use onicode_core::{
    client::{LlmClient, LlmConfig, LlmProvider},
    session::SessionStore,
    tool::ToolRegistry,
};
use onicode_tools::{
    BashTool, EditTool, GhPrChecksTool, GhPrCreateTool, GhPrMergeTool, GhPrStatusTool, GlobTool,
    GrepTool, LsTool, ReadTool, WriteTool,
};
use tracing_subscriber::EnvFilter;

mod onboarding;

#[derive(Parser)]
#[command(name = "oni", version, about = "Open source terminal coding agent", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Prompt to send to the agent
    #[arg(index = 1)]
    prompt: Option<String>,

    /// Model to use
    #[arg(short, long, env = "ONICODE_MODEL")]
    model: Option<String>,

    /// Provider (anthropic, openai, google, ollama, openrouter, huggingface, z.ai, alibaba, x.ai, custom)
    #[arg(long, env = "ONICODE_PROVIDER")]
    provider: Option<String>,

    /// API key
    #[arg(long, env = "ONICODE_API_KEY")]
    api_key: Option<String>,

    /// Base URL for API
    #[arg(long)]
    base_url: Option<String>,

    /// Working directory
    #[arg(short = 'C', long)]
    work_dir: Option<String>,

    /// Permission mode
    #[arg(long, default_value = "auto")]
    permission_mode: String,

    /// Maximum conversation turns
    #[arg(long)]
    max_turns: Option<usize>,

    /// Print mode (non-interactive)
    #[arg(short, long)]
    print: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Debug mode
    #[arg(long)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive mode
    Run,
    /// List available tools
    Tools,
    /// List available agents
    Agents,
    /// List available skills
    Skills,
    /// List saved sessions
    Sessions,
    /// Show configuration info
    Doctor,
    /// Run setup wizard to configure provider and API key
    Setup,
    /// Generate shell completions
    Completions {
        /// Shell type
        shell: clap_complete::Shell,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    init_logging(&cli);

    let work_dir = cli
        .work_dir
        .clone()
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .map(|d| d.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| ".".to_string());

    let work_path = PathBuf::from(&work_dir);

    if !work_path.exists() {
        anyhow::bail!("Working directory does not exist: {work_dir}");
    }

    match cli.command {
        Some(Commands::Tools) => cmd_tools(&work_path).await?,
        Some(Commands::Agents) => cmd_agents(&work_path).await?,
        Some(Commands::Skills) => cmd_skills(&work_path).await?,
        Some(Commands::Sessions) => cmd_sessions(&work_path).await?,
        Some(Commands::Doctor) => cmd_doctor(&work_path, &cli).await?,
        Some(Commands::Setup) => cmd_setup(&work_path).await?,
        Some(Commands::Completions { shell }) => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "oni", &mut std::io::stdout());
            return Ok(());
        }
        Some(Commands::Run) | None => {
            onboarding::run_splash()?;

            if let Some(ref prompt) = cli.prompt {
                cmd_one_shot(&work_path, prompt, &cli).await?;
            } else {
                ensure_configured(&work_path, &cli).await?;
                cmd_interactive(&work_path, &cli).await?;
            }
        }
    }

    Ok(())
}

fn init_logging(cli: &Cli) {
    if cli.debug || cli.verbose {
        let filter = if cli.debug { "debug" } else { "info" };

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| filter.into()))
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    }
}

async fn cmd_tools(_work_path: &PathBuf) -> anyhow::Result<()> {
    let tools = get_builtin_tools(".");
    println!("Available tools:\n");
    for info in tools.list() {
        println!("  {:20} {}", info.name, info.description);
    }
    Ok(())
}

async fn cmd_agents(work_path: &PathBuf) -> anyhow::Result<()> {
    let config = OniCodeConfig::load(work_path).await;
    println!("Available agents:\n");
    for (name, agent) in &config.agents {
        println!("  {:20} {}", name, agent.description);
    }
    Ok(())
}

async fn cmd_skills(work_path: &PathBuf) -> anyhow::Result<()> {
    let config = OniCodeConfig::load(work_path).await;
    if config.skills.is_empty() {
        println!("No skills found. Create .onicode/skills/<name>/SKILL.md");
    } else {
        println!("Available skills:\n");
        for skill in &config.skills {
            println!("  {:20} {}", skill.name, skill.description);
        }
    }
    Ok(())
}

async fn cmd_sessions(_work_path: &PathBuf) -> anyhow::Result<()> {
    let store_path = dirs::data_local_dir()
        .map(|d| d.join("onicode").join("sessions"))
        .unwrap_or_else(|| PathBuf::from(".onicode/sessions"));

    if !store_path.exists() {
        println!("No saved sessions found.");
        return Ok(());
    }

    println!("Saved sessions:\n");
    for entry in std::fs::read_dir(&store_path)? {
        let entry = entry?;
        println!("  {}", entry.file_name().to_string_lossy());
    }
    Ok(())
}

async fn cmd_doctor(work_path: &PathBuf, cli: &Cli) -> anyhow::Result<()> {
    println!("OniCode Doctor\n");

    println!("  Version:      {}", env!("CARGO_PKG_VERSION"));
    println!("  Working dir:  {}", work_path.display());

    let has_anthropic = std::env::var("ANTHROPIC_API_KEY").is_ok() || cli.api_key.is_some();
    let has_openai = std::env::var("OPENAI_API_KEY").is_ok();
    let has_google = std::env::var("GOOGLE_AI_API_KEY").is_ok();
    let has_openrouter = std::env::var("OPENROUTER_API_KEY").is_ok();
    let has_huggingface =
        std::env::var("HF_TOKEN").is_ok() || std::env::var("HUGGINGFACE_API_KEY").is_ok();
    let has_zai = std::env::var("ZAI_API_KEY").is_ok();
    let has_alibaba =
        std::env::var("DASHSCOPE_API_KEY").is_ok() || std::env::var("ALIBABA_API_KEY").is_ok();
    let has_xai = std::env::var("XAI_API_KEY").is_ok();

    println!("\n  API Keys:");
    println!(
        "    Anthropic:    {}",
        if has_anthropic { "found" } else { "not set" }
    );
    println!(
        "    OpenAI:       {}",
        if has_openai { "found" } else { "not set" }
    );
    println!(
        "    Google:       {}",
        if has_google { "found" } else { "not set" }
    );
    println!(
        "    OpenRouter:   {}",
        if has_openrouter { "found" } else { "not set" }
    );
    println!(
        "    HuggingFace:  {}",
        if has_huggingface { "found" } else { "not set" }
    );
    println!(
        "    z.ai:         {}",
        if has_zai { "found" } else { "not set" }
    );
    println!(
        "    Alibaba:      {}",
        if has_alibaba { "found" } else { "not set" }
    );
    println!(
        "    x.ai:         {}",
        if has_xai { "found" } else { "not set" }
    );

    let config = OniCodeConfig::load(work_path).await;
    println!("\n  Config dir:   {}", config.config_dir.display());
    println!("  Agents:       {}", config.agents.len());
    println!("  Skills:       {}", config.skills.len());
    println!("  Hooks:        {}", config.hooks.len());
    println!("  Cron jobs:    {}", config.cron_jobs.len());
    println!("  MCP servers:  {}", config.mcp_config.servers.len());

    println!("\n  Settings:");
    println!("    Model:          {}", config.settings.model);
    println!("    Max turns:      {}", config.settings.max_turns);
    println!("    Permission:     {}", config.settings.permission_mode);
    println!("    Auto compact:   {}", config.settings.auto_compact);

    println!("\n  Status: OK");
    Ok(())
}

async fn cmd_setup(_work_path: &PathBuf) -> anyhow::Result<()> {
    onboarding::run_onboarding()?;
    Ok(())
}

async fn cmd_one_shot(work_path: &PathBuf, prompt: &str, cli: &Cli) -> anyhow::Result<()> {
    let config = OniCodeConfig::load(work_path).await;
    let llm_client = create_llm_client(&config, cli)?;
    let mut tools = get_builtin_tools(&work_path.to_string_lossy());

    // Register MCP tool placeholders
    for (_server, name, desc, schema) in load_mcp_tools(&config) {
        tools.register_mcp_tool(&name, &desc, schema);
    }

    let client = Arc::new(llm_client);
    let tools = Arc::new(tools);

    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);

    // Consume events in background to avoid channel blocking
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                onicode_core::agent::AgentEvent::ToolCall { name, input } => {
                    eprintln!("[tool] {name}: {input}");
                }
                onicode_core::agent::AgentEvent::ToolResult { name, is_error, .. } => {
                    let status = if is_error { "error" } else { "ok" };
                    eprintln!("[tool:{status}] {name}");
                }
                onicode_core::agent::AgentEvent::Error(text) => {
                    eprintln!("[error] {text}");
                }
                onicode_core::agent::AgentEvent::Thinking(text) => {
                    eprintln!("[thinking] {text}");
                }
                _ => {}
            }
        }
    });

    let agent = onicode_core::Agent::new(
        client.clone(),
        tools.clone(),
        cli.max_turns.unwrap_or(config.settings.max_turns),
        event_tx,
    );

    let mut session = onicode_core::session::Session::new(
        work_path.to_string_lossy().to_string(),
        client.config().model.clone(),
    );

    if let Some(ctx) = onicode_core::load_project_context(work_path) {
        session.add_message(onicode_core::Message::system(&ctx));
    }

    agent.run(&mut session, prompt).await?;

    // Print final response
    for msg in &session.messages {
        if matches!(msg.role, onicode_core::MessageRole::Assistant) {
            if !msg.content.is_empty() {
                println!("{}", msg.content);
            }
        }
    }

    Ok(())
}

async fn cmd_interactive(work_path: &PathBuf, cli: &Cli) -> anyhow::Result<()> {
    let config = OniCodeConfig::load(work_path).await;
    let llm_client = create_llm_client(&config, cli)?;
    let mut tools = get_builtin_tools(&work_path.to_string_lossy());

    // Register MCP tool placeholders
    for (_server, name, desc, schema) in load_mcp_tools(&config) {
        tools.register_mcp_tool(&name, &desc, schema);
    }

    let client = Arc::new(llm_client);
    let tools = Arc::new(tools);

    let question_bridge = onicode_core::QuestionBridge::new();

    let agent_mode = onicode_core::agent::AgentMode::from_str(&config.settings.agent_mode);

    let app_config = onicode_tui::AppConfig {
        client: client.clone(),
        tools,
        max_turns: cli.max_turns.unwrap_or(config.settings.max_turns),
        agent_mode,
        permissions: Arc::new(onicode_core::PermissionManager::new(
            onicode_config::PermissionMode::from_str(&config.settings.permission_mode),
        )),
        working_directory: work_path.to_string_lossy().to_string(),
        question_bridge,
        hooks: config.hooks.clone(),
    };

    let (mut app, _agent_tx) = onicode_tui::App::new(app_config);

    if let Some(ctx) = onicode_core::load_project_context(work_path) {
        app.inject_system_context(&ctx);
    }

    // Show loading screen before TUI starts
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    onboarding::show_loading_screen(&mut out)?;

    app.run().await?;

    Ok(())
}

async fn ensure_configured(work_path: &PathBuf, cli: &Cli) -> anyhow::Result<()> {
    let has_cli_provider = cli.provider.is_some() || cli.api_key.is_some() || cli.model.is_some();

    if has_cli_provider {
        return Ok(());
    }

    let settings = Settings::load(work_path).await;
    let is_default = settings.provider == "anthropic" && settings.model == "claude-sonnet-4-6";

    if !is_default {
        return Ok(());
    }

    let has_env_key = std::env::var("ANTHROPIC_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("GOOGLE_AI_API_KEY").is_ok()
        || std::env::var("OPENROUTER_API_KEY").is_ok()
        || std::env::var("HF_TOKEN").is_ok()
        || std::env::var("ZAI_API_KEY").is_ok()
        || std::env::var("DASHSCOPE_API_KEY").is_ok()
        || std::env::var("XAI_API_KEY").is_ok()
        || std::env::var("CUSTOM_API_KEY").is_ok();

    if has_env_key {
        return Ok(());
    }

    let config_dir = work_path.join(".onicode");
    let global_config = dirs::config_dir()
        .map(|d| d.join("onicode"))
        .unwrap_or_else(|| PathBuf::from(".onicode"));

    let has_local_config = config_dir.join("settings.json").exists()
        || config_dir.join("settings.local.json").exists();
    let has_global_config = global_config.join("settings.json").exists();
    let has_api_keys =
        global_config.join("api_keys.json").exists() || config_dir.join("api_keys.json").exists();

    if (has_local_config || has_global_config) && has_api_keys {
        return Ok(());
    }

    onboarding::run_onboarding()?;

    Ok(())
}

fn create_llm_client(config: &OniCodeConfig, cli: &Cli) -> anyhow::Result<LlmClient> {
    let provider = match cli.provider.as_deref().unwrap_or(&config.settings.provider) {
        "anthropic" => LlmProvider::Anthropic,
        "openai" => LlmProvider::Openai,
        "google" => LlmProvider::Google,
        "ollama" => LlmProvider::Ollama,
        "bedrock" => LlmProvider::Bedrock,
        "vertex" => LlmProvider::Vertex,
        "openrouter" => LlmProvider::OpenRouter,
        "huggingface" => LlmProvider::HuggingFace,
        "z.ai" | "zai" => LlmProvider::Zai,
        "alibaba" | "dashscope" | "qwen" => LlmProvider::Alibaba,
        "x.ai" | "xai" | "grok" => LlmProvider::Xai,
        "custom" => LlmProvider::Custom,
        other => anyhow::bail!("Unknown provider: {other}"),
    };

    let api_key = cli
        .api_key
        .clone()
        .or_else(|| match provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
            LlmProvider::Openai => std::env::var("OPENAI_API_KEY").ok(),
            LlmProvider::Google => std::env::var("GOOGLE_AI_API_KEY").ok(),
            LlmProvider::Ollama => Some("unused".to_string()),
            LlmProvider::OpenRouter => std::env::var("OPENROUTER_API_KEY").ok(),
            LlmProvider::HuggingFace => std::env::var("HF_TOKEN")
                .ok()
                .or_else(|| std::env::var("HUGGINGFACE_API_KEY").ok()),
            LlmProvider::Zai => std::env::var("ZAI_API_KEY").ok(),
            LlmProvider::Alibaba => std::env::var("DASHSCOPE_API_KEY")
                .ok()
                .or_else(|| std::env::var("ALIBABA_API_KEY").ok()),
            LlmProvider::Xai => std::env::var("XAI_API_KEY").ok(),
            LlmProvider::Custom => std::env::var("CUSTOM_API_KEY").ok(),
            _ => None,
        })
        .or_else(|| onboarding::load_api_key_from_file(&provider.to_string()))
        .ok_or_else(|| anyhow::anyhow!("No API key found for provider {provider}"))?;

    let model = cli
        .model
        .clone()
        .unwrap_or_else(|| config.settings.model.clone());

    let llm_config = LlmConfig {
        provider,
        model,
        api_key,
        base_url: cli.base_url.clone().or(config.settings.base_url.clone()),
        temperature: config.settings.temperature,
        max_tokens: config.settings.max_tokens,
    };

    Ok(LlmClient::new(llm_config))
}

fn get_builtin_tools(work_dir: &str) -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    registry.register(BashTool::new(work_dir.to_string(), 30));
    registry.register(ReadTool::new(work_dir.to_string(), 200));
    registry.register(WriteTool::new(work_dir.to_string()));
    registry.register(EditTool::new(work_dir.to_string()));
    registry.register(GlobTool::new(work_dir.to_string()));
    registry.register(GrepTool::new(work_dir.to_string(), 100));
    registry.register(LsTool::new(work_dir.to_string()));
    registry.register(GhPrCreateTool);
    registry.register(GhPrMergeTool);
    registry.register(GhPrStatusTool);
    registry.register(GhPrChecksTool);

    registry
}

fn load_mcp_tools(config: &OniCodeConfig) -> Vec<(String, String, String, serde_json::Value)> {
    let mut tools = Vec::new();

    for (server_name, server_entry) in &config.mcp_config.servers {
        match server_entry {
            onicode_mcp::McpServerEntry::Stdio(stdio) => {
                tracing::info!(server = server_name, "MCP server configured (stdio)");
                // MCP tools are loaded lazily via the task tool or direct invocation
                // For now, register placeholder entries so the agent knows they exist
                tools.push((
                    server_name.clone(),
                    "mcp_connect".to_string(),
                    format!(
                        "Connect to MCP server '{}' and list available tools",
                        server_name
                    ),
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                ));
            }
            onicode_mcp::McpServerEntry::Http(http) => {
                tracing::info!(server = server_name, url = %http.url, "MCP server configured (http)");
                tools.push((
                    server_name.clone(),
                    "mcp_connect".to_string(),
                    format!(
                        "Connect to MCP server '{}' at {} and list available tools",
                        server_name, http.url
                    ),
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                ));
            }
        }
    }

    tools
}
