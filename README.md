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

## Hướng Dẫn Cài Đặt & Sử Dụng (A-Z)

### Bước 1: Cài Đặt Dependencies
```bash
# Cài Rust (nếu chưa có)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify
rustc --version
cargo --version
```

### Bước 2: Clone & Build
```bash
# Clone repository
git clone https://github.com/0xboji/pocketclaw-rs
cd picoclaw-rs

# Build tất cả components (CLI, Server, Supervisor)
cargo build --release

# Binaries sẽ nằm trong target/release/:
# - pocketclaw-cli
# - pocketclaw-server  
# - pocketclaw-supervisor
```

### Bước 3: Tạo Config File
```bash
# Tạo thư mục config
mkdir -p ~/.pocketclaw

# Tạo file config
cat > ~/.pocketclaw/config.json << 'EOF'
{
  "workspace": "./workspace",
  "agents": {
    "default": {
      "model": "claude-3-5-sonnet-20240620",
      "system_prompt": "You are a helpful AI assistant.",
      "max_tokens": 4096,
      "temperature": 0.7
    }
  },
  "providers": {
    "anthropic": {
      "api_key": "YOUR_ANTHROPIC_API_KEY",
      "model": "claude-3-5-sonnet-20240620"
    }
  },
  "attachment_policy": {
    "enabled": true,
    "max_size_bytes": 10485760,
    "allowed_mime_types": ["image/png", "image/jpeg", "text/plain", "application/pdf"],
    "storage_directory": "attachments"
  }
}
EOF

# Tạo workspace directory
mkdir -p workspace
```

### Bước 4: Chạy Agent

#### Option A: CLI Mode (Interactive)
```bash
./target/release/pocketclaw-cli

# Hoặc với custom config
./target/release/pocketclaw-cli --config ~/.pocketclaw/config.json
```

#### Option B: Server Mode (API Gateway)
```bash
# Chạy server trên port 3000
./target/release/pocketclaw-server --port 3000

# Test API
curl http://localhost:3000/health
curl -X POST http://localhost:3000/api/message \
  -H "Content-Type: application/json" \
  -d '{"message": "Hello, what can you do?"}'
```

#### Option C: Production Mode (với Supervisor)
```bash
# Supervisor tự động restart nếu crash
./target/release/pocketclaw-supervisor \
  --command "./target/release/pocketclaw-server" \
  --args "--port 3000" \
  --health-url "http://127.0.0.1:3000/health" \
  --health-interval-secs 30 \
  --max-fails 3

# Logs sẽ được ghi vào logs/supervisor.log
```

### Bước 5: Upload File (Attachment)
```bash
# Upload file qua API
curl -X POST http://localhost:3000/api/attachment \
  -F "file=@/path/to/image.png"

# Response:
# {
#   "id": "uuid",
#   "url": "attachment://uuid",
#   "filename": "image.png",
#   "mime_type": "image/png",
#   "size_bytes": 12345
# }
```

### Bước 6: Quản Lý Skills & Permissions
```bash
# Approve một skill
./target/release/pocketclaw-cli skills approve my-skill

# Revoke permission
./target/release/pocketclaw-cli skills revoke my-skill

# List approved skills
./target/release/pocketclaw-cli skills list
```

### Configuration Chi Tiết

#### Providers (LLM)
Hỗ trợ nhiều providers:
```json
{
  "providers": {
    "anthropic": { "api_key": "sk-ant-..." },
    "openai": { "api_key": "sk-...", "model": "gpt-4" },
    "openrouter": { "api_key": "sk-or-...", "api_base": "https://openrouter.ai/api/v1" },
    "groq": { "api_key": "gsk_..." }
  }
}
```

#### Telegram Bot (Optional)
```json
{
  "telegram": {
    "token": "YOUR_BOT_TOKEN"
  }
}
```

#### Discord Bot (Optional)
```json
{
  "discord": {
    "token": "YOUR_DISCORD_TOKEN"
  }
}
```

## Security Model
*   **Untrusted Skills**: Must be approved via `pocketclaw-cli skills approve <name>`.
*   **Network Access**: All outbound requests are filtered. Internal subnets are blocked by default.
*   **File Access**: `read_file` and `write_file` trapped within `workspace/`. Can be further restricted per skill.
