# PocketClaw (Rust + Android Local Gateway)

PocketClaw is a local-first AI agent runtime designed to run on:
- Linux/macOS servers
- Termux on Android
- Old Android phones as local gateway nodes

Main goal: users can configure an API key and start using the local gateway quickly.

## What You Get

- Local gateway API (`/api/message`, `/api/status`, `/api/monitor/metrics`, `/api/control/reload`)
- Multi-provider LLM support (OpenAI, OpenRouter, Anthropic, Google, Groq)
- Optional Telegram/Discord channels
- Tool sandboxing (filesystem boundary, exec timeout, SSRF checks)
- SQLite session persistence + audit logs
- Android app with controller-style setup screens

---

## 1) Quick Start (Desktop / Server)

### 1.1 Prerequisites

- Rust stable toolchain
- `clang` (recommended)
- `git`

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 1.2 Clone and Build

```bash
git clone https://github.com/0xBoji/microclaw.git
cd microclaw
cargo build --release
```

### 1.3 Create Config (`~/.pocketclaw/config.json`)

```bash
mkdir -p ~/.pocketclaw
mkdir -p ~/pocketclaw-workspace

cat > ~/.pocketclaw/config.json << 'JSON'
{
  "workspace": "/Users/YOUR_USER/pocketclaw-workspace",
  "agents": {
    "default": {
      "model": "gpt-4o-mini",
      "system_prompt": "You are a helpful assistant.",
      "max_tokens": 4096,
      "temperature": 0.7
    }
  },
  "providers": {
    "openai": {
      "api_key": "YOUR_OPENAI_API_KEY",
      "model": "gpt-4o-mini"
    }
  }
}
JSON
```

Replace:
- `workspace` with your real absolute path
- `YOUR_OPENAI_API_KEY` with your key

### 1.4 Start Gateway

```bash
./target/release/pocketclaw-cli gateway
```

### 1.5 Test API

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/status

curl -X POST http://127.0.0.1:8080/api/message \
  -H "Content-Type: application/json" \
  -d '{"message":"Hello from local gateway"}'
```

---

## 2) Termux Setup (Android CLI Mode)

Use this if you want to run PocketClaw directly inside Termux.

### 2.1 Install Dependencies

```bash
pkg update && pkg upgrade -y
pkg install -y git curl clang make pkg-config openssl
```

Install Rust in Termux:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2.2 Build

```bash
git clone https://github.com/0xBoji/microclaw.git
cd microclaw
cargo build --release
```

### 2.3 Create Config

```bash
mkdir -p ~/.pocketclaw
mkdir -p ~/pocketclaw-workspace

cat > ~/.pocketclaw/config.json << 'JSON'
{
  "workspace": "/data/data/com.termux/files/home/pocketclaw-workspace",
  "agents": {
    "default": {
      "model": "gpt-4o-mini",
      "system_prompt": "You are a helpful assistant.",
      "max_tokens": 4096,
      "temperature": 0.7
    }
  },
  "providers": {
    "openai": {
      "api_key": "YOUR_OPENAI_API_KEY",
      "model": "gpt-4o-mini"
    }
  }
}
JSON
```

### 2.4 Run Gateway in Termux

```bash
./target/release/pocketclaw-cli gateway
```

If you need 24/7 process management, use `tmux` or run via `pocketclaw-supervisor`.

---

## 3) Android App Setup (Old Phones as Local Gateway)

PocketClaw includes an Android app and JNI bridge for local gateway control.

### 3.1 Build Native + APK

Requirements:
- Android Studio (recommended)
- Android SDK + NDK
- Rust + `cargo-ndk`

Build native libs:

```bash
./build_android.sh
```

Then build APK:

```bash
cd android
# Use Android Studio or Gradle wrapper (if present in your env)
gradle assembleDebug
```

### 3.2 Install and Configure in App

The app provides controller screens:
1. Workspace Creator
2. Provider & Secrets Manager
3. Channel Chat Setup
4. Skill Manifest Viewer & Permissions
5. Agent Control Dashboard (Start/Stop)
6. Resource & Log Monitor
7. Safety & Sandbox Toggles

Minimum required input:
- Provider
- API key
- Model

Then tap Start in dashboard.

---

## 4) Common CLI Commands

```bash
# Interactive agent message
./target/release/pocketclaw-cli agent -m "Summarize my workspace"

# Gateway
./target/release/pocketclaw-cli gateway

# Status
./target/release/pocketclaw-cli status

# Onboarding wizard
./target/release/pocketclaw-cli onboard

# Cron
./target/release/pocketclaw-cli cron list
./target/release/pocketclaw-cli cron add --name "heartbeat" --message "check tasks" --every 300
```

---

## 5) Configuration Notes

### 5.1 Providers

Example OpenRouter section:

```json
{
  "providers": {
    "openrouter": {
      "api_key": "YOUR_OPENROUTER_KEY",
      "api_base": "https://openrouter.ai/api/v1",
      "model": "openai/gpt-4o-mini"
    }
  }
}
```

### 5.2 Optional Integrations

```json
{
  "telegram": { "token": "TELEGRAM_BOT_TOKEN" },
  "discord": { "token": "DISCORD_BOT_TOKEN" },
  "web": {
    "brave_key": "BRAVE_SEARCH_API_KEY",
    "auth_token": "OPTIONAL_GATEWAY_BEARER_TOKEN"
  },
  "google_sheets": {
    "spreadsheet_id": "YOUR_SHEET_ID",
    "service_account_json": "{...service account json...}"
  }
}
```

---

## 6) Security Defaults

- If gateway auth token is not set, server binds to localhost only.
- Tools are permission-gated by approved skills.
- Filesystem tool access is constrained to workspace.
- Web fetch includes SSRF checks against private/reserved ranges.

---

## 7) Troubleshooting

### `Failed to load config`
- Check `~/.pocketclaw/config.json` exists and valid JSON.

### Android build error: `SDK location not found`
- Set `ANDROID_HOME`, or create `android/local.properties`:

```properties
sdk.dir=/absolute/path/to/Android/sdk
```

### Termux runtime issues
- Ensure `source "$HOME/.cargo/env"`
- Rebuild after updates: `cargo clean && cargo build --release`

---

## 8) Project Layout

- `crates/core` - shared types/config/security primitives
- `crates/agent` - agent loop, context building, sessions
- `crates/tools` - exec/fs/web tools and sandbox controls
- `crates/providers` - LLM provider adapters
- `crates/server` - HTTP gateway
- `crates/cli` - command entrypoint and onboarding
- `crates/mobile-jni` - Android JNI bridge
- `android/` - Android app project

---

## 9) License

MIT
