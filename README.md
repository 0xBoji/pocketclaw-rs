# PocketClaw

Ultra-lightweight personal AI assistant written in Rust.

PocketClaw is a modular, terminal-first AI agent that connects to multiple LLM providers and messaging platforms. It supports tools, skills, scheduled tasks, and voice transcription out of the box.

## Requirements

- Rust 1.75 or later
- Cargo (included with Rust)

## Installation

```bash
git clone https://github.com/0xBoji/pocketclaw-rs.git
cd pocketclaw-rs
cargo build --release
```

The compiled binary will be at `target/release/pocketclaw-cli`.

## Quick Start

Run the onboarding wizard to create your configuration:

```bash
pocketclaw onboard
```

This will prompt you for:

- Workspace directory
- LLM provider API keys (OpenAI, Anthropic, Google Gemini, Groq)
- Agent settings (name, model, temperature)
- Integration tokens (Telegram, Discord)
- Web search API key (Brave)

The configuration is saved to `~/.pocketclaw/config.json`. The wizard also creates workspace template files (AGENTS.md, SOUL.md, USER.md, TOOLS.md, IDENTITY.md, and memory/MEMORY.md) in your chosen workspace directory.

## Usage

### Send a message

```bash
pocketclaw agent -m "What is the weather today?"
```

### Start the gateway server

```bash
pocketclaw gateway
```

This starts the HTTP server on port 8080, along with any configured Telegram and Discord bots, the heartbeat service, and the cron scheduler.

**Endpoints:**

| Path          | Method | Description          |
|:--------------|:-------|:---------------------|
| `/health`     | GET    | Health check         |
| `/api/status` | GET    | Service status (JSON)|

### Check status

```bash
pocketclaw status
```

Displays the current configuration state, including which providers and integrations are configured.

### Manage scheduled tasks

```bash
# List all jobs
pocketclaw cron list

# Add a recurring job (every 3600 seconds)
pocketclaw cron add --name "daily-summary" --message "Summarize today's events" --every 3600

# Remove a job
pocketclaw cron remove <job-id>

# Enable or disable a job
pocketclaw cron enable <job-id>
pocketclaw cron disable <job-id>
```

### Manage skills

```bash
# List installed skills
pocketclaw skills list

# Show skill details
pocketclaw skills show <skill-name>
```

Skills are directories inside your workspace's `skills/` folder. Each skill should contain a `SKILL.md` file describing its purpose and instructions.

### Print version

```bash
pocketclaw --version
```

## Configuration

The configuration file is located at `~/.pocketclaw/config.json`. You can edit it directly or re-run `pocketclaw onboard` to regenerate it.

### Example configuration

```json
{
  "workspace": "/path/to/workspace",
  "agents": {
    "default": {
      "model": "gpt-4o",
      "system_prompt": "You are a helpful assistant.",
      "max_tokens": 4096,
      "temperature": 0.7
    }
  },
  "providers": {
    "openai": {
      "api_key": "sk-...",
      "api_base": "https://api.openai.com/v1",
      "model": "gpt-4o"
    },
    "anthropic": {
      "api_key": "sk-ant-...",
      "model": "claude-3-5-sonnet-20241022"
    },
    "google": {
      "api_key": "...",
      "model": "gemini-1.5-flash"
    },
    "groq": {
      "api_key": "gsk_..."
    }
  },
  "telegram": {
    "token": "123456:ABC..."
  },
  "discord": {
    "token": "..."
  },
  "web": {
    "brave_key": "..."
  }
}
```

## Supported LLM Providers

| Provider      | Models                          | Config Key   |
|:--------------|:--------------------------------|:-------------|
| OpenAI        | gpt-4o, gpt-4o-mini, etc.      | `openai`     |
| OpenRouter    | Any model via OpenRouter        | `openrouter` |
| Anthropic     | Claude 3.5 Sonnet, Opus, Haiku | `anthropic`  |
| Google Gemini | gemini-1.5-flash, gemini-pro   | `google`     |
| Groq          | whisper-large-v3 (voice only)  | `groq`       |

## Built-in Tools

| Tool        | Description                              |
|:------------|:-----------------------------------------|
| exec        | Execute shell commands in the workspace  |
| read_file   | Read file contents                       |
| write_file  | Write content to a file                  |
| list_dir    | List directory contents                  |
| web_fetch   | Fetch content from a URL                 |
| web_search  | Search the web via Brave Search API      |

## Architecture

Microclaw is organized as a Cargo workspace with 11 crates:

```
pocketclaw-rs/
  crates/
    core/          # Config, MessageBus, types
    agent/         # Agent loop, context builder, session management
    providers/     # LLM provider implementations
    tools/         # Tool trait and built-in tools
    skills/        # Skill loading from workspace
    server/        # HTTP gateway (Axum)
    telegram/      # Telegram bot integration (Teloxide)
    discord/       # Discord bot integration (Serenity)
    cron/          # Scheduled task service
    voice/         # Groq Whisper transcription
    heartbeat/     # Periodic health check service
    cli/           # CLI entry point (Clap)
```

### Key design decisions

- **Modular crates**: Each concern is isolated in its own crate with explicit dependencies.
- **Async runtime**: Tokio is used throughout for non-blocking I/O.
- **Message bus**: A broadcast-based event bus decouples inbound and outbound message handling.
- **Provider abstraction**: A common `LLMProvider` trait allows swapping providers without changing agent logic.
- **Config-free commands**: Commands like `onboard`, `status`, and `cron` work without a pre-existing configuration file.

## Workspace Files

When you run `pocketclaw onboard`, the following template files are created in your workspace:

| File               | Purpose                                      |
|:-------------------|:---------------------------------------------|
| `AGENTS.md`        | Agent behavior instructions and capabilities |
| `SOUL.md`          | Personality and communication style          |
| `USER.md`          | Information about the user                   |
| `TOOLS.md`         | Available tools and usage guidelines         |
| `IDENTITY.md`      | Agent identity and role definition           |
| `memory/MEMORY.md` | Persistent memory and notes                  |

These files are loaded automatically by the context builder when constructing prompts for the LLM.

## Development

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run directly
cargo run -- agent -m "Hello"

# Check for errors without building
cargo check
```

## License

MIT
