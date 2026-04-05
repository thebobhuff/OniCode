# OniCode

Open source terminal-based AI coding agent with tight terminal integration across Windows, macOS, and Linux.

## Features

- Multi-provider LLM support (Anthropic, OpenAI, Google, local models)
- 25+ built-in coding tools
- MCP server integration (stdio, SSE, Streamable HTTP, WebSocket)
- Event-driven hook system
- Scheduled cron jobs
- Declarative skills and custom agents
- Sandboxed shell execution via portable-pty
- Full TUI with split panels, syntax highlighting, and mouse support

## Quick Start

```bash
# Build from source
cargo build --release

# Run
oni "explain this codebase"

# Interactive mode
oni
```

## Architecture

```
crates/
├── onicode/       # Binary entry point, wiring everything together
├── core/          # Agent loop, tool trait, LLM clients, session management
├── tui/           # Ratatui UI, crossterm events, panels, rendering
├── tools/         # Built-in tools (Bash, Read, Write, Edit, Glob, Grep, LS...)
├── mcp/           # MCP client with all 4 transports
└── config/        # Settings, hooks, cron, agents, skills, config loading
```

## Extension System

| Layer | Mechanism | Example |
|---|---|---|
| Skills/Agents | Markdown + YAML frontmatter | `.onicode/agents/reviewer.md` |
| Hooks | Shell commands on events | `.onicode/hooks.json` |
| MCP Servers | Any language via stdio/HTTP | `.mcp.json` |
| Built-in Tools | Rust (compiled in) | Bash, Read, Write, Edit... |

## License

MIT
