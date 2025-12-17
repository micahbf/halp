# halp

A fast terminal utility that translates natural language into shell commands using LLMs.

## Prerequisites

You must have Rust installed. See: [Install Rust](https://rust-lang.org/tools/install/).

## Installation

Clone this repo and run:

```bash
cargo install --path .
```

## Usage

```bash
halp rsync all files to remote server preserving permissions
# Output: rsync -av ./ user@remote:/path/

halp find all rust files larger than 1mb
# Output: find . -name "*.rs" -size +1M

halp compress this directory excluding node_modules
# Output: tar -czvf archive.tar.gz --exclude='node_modules' .
```

### Options

```
halp [OPTIONS] <QUERY>...

Arguments:
  <QUERY>...  Natural language description of the command you need

Options:
  -q, --quiet    Suppress explanation (command only)
  -e, --explain  Show explanation only (no command output)
  -h, --help     Print help
  -V, --version  Print version
```

### Shell Integration

For seamless usage, add a wrapper function to your shell config:

**zsh** (`~/.zshrc`):

```zsh
function _halp() { print -z "$(halp "$*")" }
alias h='noglob _halp'
```

**bash** (`~/.bashrc`):

```bash
function h() { read -e -i "$(halp "$@")" cmd && eval "$cmd"; }
```

This lets you type `h list files by size` and have the command inserted at your prompt for review before execution.

## Configuration

Configuration is loaded in this priority order:

### 1. HALP-specific Environment Variables (highest priority)

```bash
export HALP_PROVIDER=anthropic    # or "openai" or "gemini"
export HALP_MODEL=claude-haiku-4-5
export HALP_API_KEY=sk-ant-...
```

### 2. Config File

`~/.config/halp/config.toml`:

```toml
provider = "anthropic"  # or "openai" or "gemini"
model = "claude-haiku-4-5"
api_key = "sk-ant-..."

# Optional: custom API endpoint
# api_base_url = "https://api.anthropic.com/v1/messages"

# Optional: custom system prompt with {{os}}, {{shell}}, {{cwd}} template variables.
# Below is the default prompt - uncomment and modify as needed.
# system_prompt = """
# You are a command-line assistant. Generate a shell command for the user's request.
#
# Format your response EXACTLY as:
# COMMAND: <the exact command to run>
# EXPLANATION: <brief one-line explanation>
#
# Context:
# - OS: {{os}}
# - Shell: {{shell}}
# - Working directory: {{cwd}}
#
# Rules:
# - Output exactly one command (use && or ; for multi-step operations)
# - The command must be valid for the specified OS and shell
# - Prefer common, portable commands when possible
# - Keep explanation to one concise line
# - Never include dangerous commands (rm -rf /, etc) without explicit confirmation flags
# - If the request is ambiguous, make a reasonable assumption and note it in the explanation
# """
```

### 3. Provider-specific Environment Variables (fallback)

If no API key is set via `HALP_API_KEY` or config file, halp falls back to:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
# or
export OPENAI_API_KEY=sk-...
# or
export GEMINI_API_KEY=...
```

### Supported Providers

| Provider    | Default Model      | Environment Variable |
| ----------- | ------------------ | -------------------- |
| `anthropic` | `claude-haiku-4-5` | `ANTHROPIC_API_KEY`  |
| `openai`    | `gpt-5-nano`       | `OPENAI_API_KEY`     |
| `gemini`    | `gemini-2.5-flash` | `GEMINI_API_KEY`     |

## Output Behavior

- **stdout**: The command only (for piping)
- **stderr**: Explanation streamed in real-time (dimmed text)

This design allows easy integration with shell functions and pipes:

```bash
# Capture just the command
cmd=$(halp list large files)

# Pipe to clipboard
halp git squash last 3 commits | pbcopy
```

## Examples

```bash
# File operations
halp find all json files modified in the last week
halp delete all .DS_Store files recursively

# Git
halp undo the last commit but keep changes
halp show diff between main and this branch

# System
halp show disk usage sorted by size
halp kill process on port 3000

# Networking
halp download this url and save as output.zip
halp check if port 443 is open on google.com
```

## License

MIT
