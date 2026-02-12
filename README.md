# PocketClaw (Rust Edition)

PocketClaw is a secure, high-performance AI agent runtime designed for mobile (Android/Termux) and server environments. It features a sandboxed execution environment, robust permission system, and multi-channel support (CLI, Telegram, Discord).

## Features

### 🛡️ Security & Sandboxing (Wave A)
*   **Path Isolation**: All file operations are strictly confined to the workspace. Symlinks and `..` traversal are blocked.
*   **Process Sandboxing**: Tools run in isolated process groups. Timeouts kill the entire process tree to prevent zombies.
*   **Network Guard**: SSRF protection blocks access to private IPs (localhost, 192.168.x.x, AWS metadata).
*   **Permission System**: granular `skill.toml` manifest system. Default-deny policy for all tools.
*   **Secret Management**: Secrets are stored in `~/.pocketclaw/secrets.json` (0600 permissions) and masked in logs.

### 🏗️ Architecture & Reliability (Wave B)
*   **SQLite Persistence**: Robust session and message storage (replacing fragile JSON files).
*   **Unified Audit Logging**: Structured `audit.jsonl` tracks every tool execution, error, and security event.
*   **Cron Security**: Scheduled tasks run with strict identity tagging and security boundaries.
*   **Cost Control**: Auto-summarization and history trimming (keep last 10 messages) to manage context window limits.

### 🚀 Production Ready (Wave C)
*   **Backpressure**: Dedicated inbound message queue prevents system logs from flooding agent commands.
*   **Resource Limits**: Enforced `cpu`, `nproc`, and `nofile` limits via `setrlimit` (Unix/Android).
*   **Attachment Policy**: Secure file uploads with MIME type validation, size limits, and isolated storage.
*   **Supervisor**: Built-in watchdog binary (`pocketclaw-supervisor`) for auto-restart, healthchecks, and log rotation.

### 📱 Android Integration
*   Native JNI bindings for Android.
*   Optimization for Termux environments (binary size < 20MB).

## Usage

### Installation
```bash
# Clone repository
git clone https://github.com/0xboji/pocketclaw-rs
cd picoclaw-rs

# Build all components
cargo build --release
```

### Running the Agent (CLI)
```bash
# Start interactive session
./target/release/pocketclaw-cli
```

### Running the Server (Gateway)
```bash
# Start API server
./target/release/pocketclaw-server --port 3000
```

### Supervisor (Recommended for Production)
The supervisor manages the server process, restarts it on crash, and rotates logs.
```bash
./target/release/pocketclaw-supervisor \
  --command "./target/release/pocketclaw-server" \
  --health-url "http://127.0.0.1:3000/health"
```

### Configuration
Config is stored in `~/.pocketclaw/config.json`.
```json
{
  "workspace": "/path/to/workspace",
  "agents": {
    "default": {
      "model": "claude-3-5-sonnet-20240620",
      "temperature": 0.7
    }
  },
  "providers": {
    "anthropic": { "api_key": "sk-..." }
  },
  "attachment_policy": {
    "enabled": true,
    "max_size_bytes": 10485760,
    "allowed_mime_types": ["image/png", "text/plain"]
  }
}
```

## Security Model
*   **Untrusted Skills**: Must be approved via `pocketclaw-cli skills approve <name>`.
*   **Network Access**: All outbound requests are filtered. Internal subnets are blocked by default.
*   **File Access**: `read_file` and `write_file` trapped within `workspace/`. Can be further restricted per skill.
