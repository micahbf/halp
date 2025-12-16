# CLAUDE.md

This file provides context for Claude Code when working on this project.

## Project Overview

`halp` is a Rust CLI that converts natural language queries into shell commands using LLM APIs. It's designed for speed and simplicity, outputting commands to stdout for easy shell integration.

## Architecture

```
src/
├── main.rs           # CLI entry point, arg parsing with clap
├── config.rs         # Configuration loading (env vars + XDG config file)
├── prompt.rs         # System prompt construction with OS/shell/cwd context
├── output.rs         # Response parsing and streaming output
└── providers/
    ├── mod.rs        # LlmProvider trait and factory function
    ├── anthropic.rs  # Anthropic Messages API with SSE streaming
    └── openai.rs     # OpenAI Chat Completions API with SSE streaming
```

## Key Design Decisions

1. **Output separation**: Commands go to stdout, explanations stream to stderr. This enables `print -z "$(halp ...)"` shell integration.

2. **Streaming**: Both providers stream responses via SSE. Explanation text appears in real-time on stderr while waiting for the full response.

3. **Config priority**: `HALP_*` env vars > config file > provider-specific env vars (`ANTHROPIC_API_KEY`/`OPENAI_API_KEY`). Config file takes precedence over generic provider env vars.

4. **Response format**: The LLM is prompted to respond with `COMMAND: ...` and `EXPLANATION: ...` prefixes for reliable parsing. Fallback parsing handles code blocks and raw responses.

## Build Commands

```bash
cargo build              # Dev build
cargo build --release    # Release build with LTO
cargo run -- --help      # Test CLI
cargo run -- <query>     # Run with query (needs API key)
```

## Testing Without API Key

The config module will return a clear error message if no API key is found:
```
Configuration error: No API key found. Set HALP_API_KEY, ANTHROPIC_API_KEY, or add api_key to ~/.config/halp/config.toml
```

## Adding a New Provider

1. Create `src/providers/newprovider.rs`
2. Implement the `LlmProvider` trait with `stream_completion`
3. Add the module to `src/providers/mod.rs`
4. Add variant to `Provider` enum in `src/config.rs`
5. Update the factory function in `src/providers/mod.rs`

## LLM Prompt

The system prompt in `src/prompt.rs` instructs the LLM to:
- Output exactly one command
- Use the format `COMMAND: <cmd>` and `EXPLANATION: <text>`
- Consider the user's OS, shell, and current directory
- Prefer portable commands

## Dependencies

- `tokio` - async runtime
- `reqwest` - HTTP client with streaming
- `clap` - CLI argument parsing
- `serde`/`serde_json` - JSON serialization
- `toml` - config file parsing
- `dirs` - XDG directory resolution
- `futures-util` - stream utilities
- `async-trait` - async trait support
