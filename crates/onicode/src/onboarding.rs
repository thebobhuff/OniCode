use std::{
    io::{self, Write},
    path::PathBuf,
    thread,
    time::Duration,
};

use onicode_config::Settings;

const COLORS: &[(u8, u8, u8)] = &[
    (0x37, 0x06, 0x17), // night_bordeaux
    (0x6a, 0x04, 0x0f), // black_cherry
    (0x9d, 0x02, 0x08), // oxblood
    (0xd0, 0x00, 0x00), // brick_ember
    (0xdc, 0x2f, 0x02), // red_ochre
    (0xe8, 0x5d, 0x04), // cayenne_red
    (0xf4, 0x8c, 0x06), // deep_saffron
    (0xfa, 0xa3, 0x07), // orange
    (0xff, 0xba, 0x08), // amber_flame
];

const ONICODE_ART: &str = r"
 ____  _   _ ___     ____ ___  ____  _____ 
/ __ \| \ | |_ _|   / ___/ _ \|  _ \| ____|
| |  | |  \| || |   | |  | | | | | | |  _|  
| |__| | |\  || |   | |__| |_| | |_| | |___ 
 \____/|_| \_|___|   \____\___/|____/|_____|";

const TAGLINE: &str = "Code like a Yokai — unseen, relentless, inevitable.";
const AUTHOR: &str = "By Bob Huff";

fn tc(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

fn reset() -> String {
    "\x1b[0m".to_string()
}

fn lerp_color(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f64 + (b.0 as f64 - a.0 as f64) * t) as u8,
        (a.1 as f64 + (b.1 as f64 - a.1 as f64) * t) as u8,
        (a.2 as f64 + (b.2 as f64 - a.2 as f64) * t) as u8,
    )
}

fn get_gradient_color(phase: f64, offset: f64) -> (u8, u8, u8) {
    let n = COLORS.len() as f64;
    let pos = ((phase + offset) % 1.0 + 1.0) % 1.0;
    let idx = pos * (n - 1.0);
    let i = idx.floor() as usize;
    let t = idx - i as f64;
    let next = (i + 1).min(COLORS.len() - 1);
    lerp_color(COLORS[i], COLORS[next], t)
}

fn render_gradient_frame(art: &str, phase: f64, speed: f64) -> String {
    let mut result = String::new();
    let lines: Vec<&str> = art.lines().collect();
    let total_height = lines.len() as f64;

    for (row_idx, line) in lines.iter().enumerate() {
        let row_phase = row_idx as f64 / total_height;
        for (col_idx, ch) in line.chars().enumerate() {
            if ch == ' ' {
                result.push(' ');
                continue;
            }
            let col_phase = col_idx as f64 / line.len().max(1) as f64;
            let offset = (row_phase * 0.3 + col_phase * 0.7) * speed + phase;
            let (r, g, b) = get_gradient_color(phase, offset);
            result.push_str(&tc(r, g, b));
            result.push(ch);
        }
        result.push_str(&reset());
        result.push('\n');
    }
    result
}

fn animate_splash(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\x1b[?25l")?;
    write!(out, "\x1b[H\x1b[2J")?;
    out.flush()?;

    let total_frames = 50;
    let mut stdout_buf = io::BufWriter::with_capacity(4096, io::stdout());

    for frame in 0..total_frames {
        let phase = frame as f64 / total_frames as f64;
        let speed = 1.2;

        write!(stdout_buf, "\x1b[H")?;

        writeln!(stdout_buf)?;
        writeln!(stdout_buf)?;

        let reveal = (phase / 0.5).min(1.0);
        let lines: Vec<&str> = ONICODE_ART.lines().collect();
        let visible_lines = (lines.len() as f64 * reveal).ceil() as usize;
        let visible_art = lines[..visible_lines].join("\n");

        if !visible_art.is_empty() {
            write!(
                stdout_buf,
                "{}",
                render_gradient_frame(&visible_art, phase, speed)
            )?;
        }

        if phase > 0.5 {
            let text_phase = ((phase - 0.5) / 0.5).min(1.0);
            if text_phase > 0.2 {
                writeln!(stdout_buf)?;
                let tag_color = get_gradient_color(phase, 0.3);
                writeln!(
                    stdout_buf,
                    "  {}{}{}",
                    tc(tag_color.0, tag_color.1, tag_color.2),
                    TAGLINE,
                    reset()
                )?;
            }
            if text_phase > 0.5 {
                let author_color = get_gradient_color(phase, 0.6);
                writeln!(
                    stdout_buf,
                    "  {}{}{}",
                    tc(author_color.0, author_color.1, author_color.2),
                    AUTHOR,
                    reset()
                )?;
            }
        }

        stdout_buf.flush()?;
        thread::sleep(Duration::from_millis(50));
    }

    write!(stdout_buf, "\x1b[?25h")?;
    stdout_buf.flush()
}

const LOADING_MESSAGES: &[&str] = &[
    "Summoning yokai from the digital realm...",
    "Sharpening katana for code battles...",
    "Feeding the oni some fresh tokens...",
    "Consulting the ancient scrolls of Stack Overflow...",
    "Awakening the coding demon...",
    "Polishing the virtual katana...",
    "Channeling the spirit of clean code...",
    "Unleashing the yokai upon your codebase...",
    "Whispering sweet nothings to the LLM...",
    "Preparing the digital battlefield...",
    "The yokai hungers for bugs...",
    "Aligning the chakras of your dependencies...",
    "Brewing a pot of digital sake...",
    "The coding spirit is restless...",
    " yokai.exe has entered the chat...",
];

pub fn show_loading_screen(out: &mut impl Write) -> io::Result<()> {
    write!(out, "\x1b[?25l")?;
    write!(out, "\x1b[H\x1b[2J")?;
    out.flush()?;

    use rand::Rng;
    let mut rng = rand::thread_rng();
    let num_messages = 3 + rng.gen_range(0..3);

    let mut used = std::collections::HashSet::new();
    for _ in 0..num_messages {
        let mut idx = rng.gen_range(0..LOADING_MESSAGES.len());
        while used.contains(&idx) {
            idx = (idx + 1) % LOADING_MESSAGES.len();
        }
        used.insert(idx);

        let msg = LOADING_MESSAGES[idx];
        let color_idx = rng.gen_range(0..COLORS.len());
        let (r, g, b) = COLORS[color_idx];

        write!(out, "\x1b[H\x1b[2J")?;
        writeln!(out)?;
        writeln!(out)?;
        writeln!(out)?;
        writeln!(out)?;
        writeln!(out, "  {}{}{}", tc(r, g, b), msg, reset())?;
        writeln!(out)?;
        writeln!(out)?;
        writeln!(
            out,
            "  {}Loading OniCode...{}",
            tc(0x80, 0x80, 0x80),
            reset()
        )?;
        out.flush()?;

        thread::sleep(Duration::from_millis(200 + rng.gen_range(0..300)));
    }

    write!(out, "\x1b[?25h")?;
    out.flush()
}

const PROVIDERS: &[(&str, &str, &str, &str)] = &[
    (
        "anthropic",
        "Anthropic",
        "ANTHROPIC_API_KEY",
        "https://console.anthropic.com/settings/keys",
    ),
    (
        "openai",
        "OpenAI",
        "OPENAI_API_KEY",
        "https://platform.openai.com/api-keys",
    ),
    (
        "openrouter",
        "OpenRouter",
        "OPENROUTER_API_KEY",
        "https://openrouter.ai/settings/keys",
    ),
    (
        "huggingface",
        "HuggingFace",
        "HF_TOKEN",
        "https://huggingface.co/settings/tokens",
    ),
    ("ollama", "Ollama (local)", "", "https://ollama.com"),
    ("z.ai", "z.ai", "ZAI_API_KEY", "https://z.ai"),
    (
        "alibaba",
        "Alibaba / DashScope",
        "DASHSCOPE_API_KEY",
        "https://dashscope.console.aliyun.com",
    ),
    ("x.ai", "x.ai / Grok", "XAI_API_KEY", "https://console.x.ai"),
    (
        "google",
        "Google AI",
        "GOOGLE_AI_API_KEY",
        "https://aistudio.google.com/apikey",
    ),
    ("custom", "Custom (OpenAI-compatible)", "", ""),
];

const POPULAR_MODELS: &[(&str, &str)] = &[
    ("anthropic", "claude-sonnet-4-20250514"),
    ("anthropic", "claude-opus-4-20250514"),
    ("anthropic", "claude-3-5-sonnet-20241022"),
    ("openai", "gpt-4.1"),
    ("openai", "gpt-4o"),
    ("openai", "gpt-4o-mini"),
    ("openrouter", "anthropic/claude-sonnet-4"),
    ("openrouter", "openai/gpt-4.1"),
    ("huggingface", "meta-llama/Llama-3.3-70B-Instruct"),
    ("z.ai", "glm-4-plus"),
    ("alibaba", "qwen-max"),
    ("x.ai", "grok-3"),
    ("google", "gemini-2.5-pro"),
    ("ollama", "llama3.1"),
];

pub struct OnboardingResult {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

pub fn run_splash() -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    animate_splash(&mut out)?;
    print_splash(&mut out)?;

    out.flush()
}

fn print_splash(out: &mut impl Write) -> io::Result<()> {
    writeln!(out)?;
    writeln!(
        out,
        "  {}OniCode v{}{}",
        tc(0xff, 0xba, 0x08),
        env!("CARGO_PKG_VERSION"),
        reset()
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "  {}Code like a Yokai — unseen, relentless, inevitable.{}",
        tc(0xe8, 0x5d, 0x04),
        reset()
    )?;
    writeln!(out)?;
    writeln!(out, "  {}By Bob Huff{}", tc(0x9d, 0x02, 0x08), reset())?;
    writeln!(out)?;
    out.flush()
}

pub fn run_onboarding() -> io::Result<OnboardingResult> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    print_splash(&mut out)?;
    writeln!(out, "  No configuration found. Let's get you set up.")?;
    writeln!(out)?;
    out.flush()?;

    let provider = select_provider(&mut out, &mut stdin.lock())?;
    let model = select_model(&mut out, &mut stdin.lock(), &provider)?;
    let api_key = collect_api_key(&mut out, &mut stdin.lock(), &provider)?;
    let base_url = collect_base_url(&mut out, &mut stdin.lock(), &provider)?;

    save_settings(&provider, &model, base_url.as_deref(), api_key.as_deref())?;

    print_success(&mut out, &provider, &model)?;

    Ok(OnboardingResult {
        provider,
        model,
        api_key,
        base_url,
    })
}

fn select_provider(out: &mut impl Write, stdin: &mut impl io::BufRead) -> io::Result<String> {
    writeln!(out, "  1. Choose your AI provider:")?;
    writeln!(out)?;

    for (i, (_, display, _, _)) in PROVIDERS.iter().enumerate() {
        writeln!(out, "     [{}] {}  ", i + 1, display)?;
    }

    writeln!(out)?;

    loop {
        write!(out, "  Select provider [1]: ")?;
        out.flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            return Ok(PROVIDERS[0].0.to_string());
        }

        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= PROVIDERS.len() {
                return Ok(PROVIDERS[n - 1].0.to_string());
            }
        }

        if let Some(found) = PROVIDERS.iter().find(|(id, _, _, _)| {
            input.eq_ignore_ascii_case(id) || input.eq_ignore_ascii_case(&id.replace('.', ""))
        }) {
            return Ok(found.0.to_string());
        }

        writeln!(out, "  Invalid selection. Try again.")?;
    }
}

fn select_model(
    out: &mut impl Write,
    stdin: &mut impl io::BufRead,
    provider: &str,
) -> io::Result<String> {
    writeln!(out)?;
    writeln!(out, "  2. Choose a model:")?;
    writeln!(out)?;

    let provider_models: Vec<_> = POPULAR_MODELS
        .iter()
        .filter(|(p, _)| *p == provider)
        .map(|(_, m)| m.to_string())
        .collect();

    if !provider_models.is_empty() {
        for (i, model) in provider_models.iter().enumerate() {
            writeln!(out, "     [{}] {}", i + 1, model)?;
        }
        writeln!(out)?;
    }

    loop {
        if !provider_models.is_empty() {
            write!(out, "  Select model [1] or enter a custom name: ")?;
        } else {
            write!(out, "  Enter model name: ")?;
        }
        out.flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() && !provider_models.is_empty() {
            return Ok(provider_models[0].clone());
        }

        if let Ok(n) = input.parse::<usize>() {
            if n >= 1 && n <= provider_models.len() {
                return Ok(provider_models[n - 1].clone());
            }
        }

        if !input.is_empty() {
            return Ok(input.to_string());
        }

        writeln!(out, "  Please enter a model name.")?;
    }
}

fn collect_api_key(
    out: &mut impl Write,
    stdin: &mut impl io::BufRead,
    provider: &str,
) -> io::Result<Option<String>> {
    let provider_info = PROVIDERS.iter().find(|(id, _, _, _)| *id == provider);

    if let Some((_, _, env_var, url)) = provider_info {
        if env_var.is_empty() {
            return Ok(None);
        }

        writeln!(out)?;
        writeln!(out, "  3. API Key:")?;
        writeln!(out)?;
        writeln!(out, "     Set the {} environment variable:", env_var)?;
        writeln!(out, "     {}", url)?;
        writeln!(out)?;
        writeln!(
            out,
            "     Or paste your API key below (leave empty to set env var later):"
        )?;
        writeln!(out)?;

        write!(out, "  API Key: ")?;
        out.flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        let input = input.trim().to_string();

        if !input.is_empty() {
            return Ok(Some(input));
        }
    }

    Ok(None)
}

fn collect_base_url(
    out: &mut impl Write,
    stdin: &mut impl io::BufRead,
    provider: &str,
) -> io::Result<Option<String>> {
    let needs_base_url = matches!(
        provider,
        "openrouter" | "z.ai" | "alibaba" | "x.ai" | "custom"
    );

    if !needs_base_url {
        return Ok(None);
    }

    let defaults = match provider {
        "openrouter" => "https://openrouter.ai/api/v1",
        "z.ai" => "https://api.z.ai/v1",
        "alibaba" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "x.ai" => "https://api.x.ai/v1",
        "custom" => "",
        _ => "",
    };

    writeln!(out)?;
    writeln!(out, "  4. Base URL:")?;
    writeln!(out)?;

    if !defaults.is_empty() {
        writeln!(out, "     Default: {}", defaults)?;
    }

    writeln!(out)?;
    write!(out, "  Base URL: ")?;
    out.flush()?;

    let mut input = String::new();
    stdin.read_line(&mut input)?;
    let input = input.trim().to_string();

    if input.is_empty() && !defaults.is_empty() {
        return Ok(Some(defaults.to_string()));
    }

    if input.is_empty() {
        return Ok(None);
    }

    Ok(Some(input))
}

fn save_settings(
    provider: &str,
    model: &str,
    base_url: Option<&str>,
    api_key: Option<&str>,
) -> io::Result<()> {
    let config_dir = dirs::config_dir()
        .map(|d| d.join("onicode"))
        .unwrap_or_else(|| PathBuf::from(".onicode"));

    std::fs::create_dir_all(&config_dir)?;

    let settings_path = config_dir.join("settings.json");

    let settings = Settings {
        model: model.to_string(),
        provider: provider.to_string(),
        max_turns: 100,
        permission_mode: "ask".to_string(),
        allowed_tools: None,
        disallowed_tools: None,
        permissions: None,
        auto_compact: true,
        always_thinking: false,
        temperature: None,
        max_tokens: None,
        system_prompt: None,
        base_url: base_url.map(String::from),
        add_dirs: Vec::new(),
        agent_mode: "build".to_string(),
    };

    let json = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_path, json)?;

    if let Some(key) = api_key {
        let keys_path = config_dir.join("api_keys.json");
        let mut keys: serde_json::Value = if keys_path.exists() {
            let content = std::fs::read_to_string(&keys_path)?;
            serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let env_var = get_api_key_env_var(provider);
        if let Some(env_var) = env_var {
            keys[env_var] = serde_json::json!(key);
            let json = serde_json::to_string_pretty(&keys)?;
            std::fs::write(&keys_path, json)?;
        }
    }

    Ok(())
}

fn get_api_key_env_var(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        "google" => Some("GOOGLE_AI_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "huggingface" => Some("HF_TOKEN"),
        "z.ai" | "zai" => Some("ZAI_API_KEY"),
        "alibaba" | "dashscope" | "qwen" => Some("DASHSCOPE_API_KEY"),
        "x.ai" | "xai" | "grok" => Some("XAI_API_KEY"),
        "custom" => Some("CUSTOM_API_KEY"),
        _ => None,
    }
}

pub fn load_api_key_from_file(provider: &str) -> Option<String> {
    let config_dir = dirs::config_dir().map(|d| d.join("onicode"))?;
    let keys_path = config_dir.join("api_keys.json");
    let content = std::fs::read_to_string(&keys_path).ok()?;
    let keys: serde_json::Value = serde_json::from_str(&content).ok()?;

    let env_var = get_api_key_env_var(provider)?;
    keys[env_var].as_str().map(String::from)
}

fn print_success(out: &mut impl Write, provider: &str, model: &str) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "  Configuration saved!")?;
    writeln!(out, "    Provider: {}", provider)?;
    writeln!(out, "    Model:    {}", model)?;
    writeln!(out)?;
    writeln!(
        out,
        "  You can change these later in ~/.config/onicode/settings.json"
    )?;
    writeln!(
        out,
        "  Or use flags: --provider {} --model {}",
        provider, model
    )?;
    writeln!(out)?;
    out.flush()
}
