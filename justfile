# OniCode Development Commands

## Build
```bash
just build          # Debug build
just release        # Release build
just check          # Fast check without compiling
```

## Test
```bash
just test           # Run all tests
just test-core      # Run core crate tests
just test-tools     # Run tools crate tests
just test-mcp       # Run MCP crate tests
```

## Lint & Format
```bash
just lint           # Clippy + format check
just fmt            # Format all files
just fix            # Auto-fix clippy warnings
```

## Run
```bash
just run            # Run in debug mode
just run-prompt "explain this codebase"  # One-shot mode
just interactive    # Interactive REPL
```

## Clean
```bash
just clean          # Remove target directory
just distclean      # Full clean including Cargo.lock
```
