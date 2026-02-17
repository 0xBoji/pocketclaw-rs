# PhoneClaw (Android-First Local Gateway)

`phoneclaw-rs` is a local-first AI agent runtime focused on Android devices, especially old phones used as always-on local gateway nodes.

Primary goal: user only needs to set `provider + api key + model`, then start the gateway.

## Highlights

- Android-first setup flow (Wizard + dashboard control)
- Multi-channel gateway (Telegram/Discord/Slack/WhatsApp/Teams/Zalo/Google Chat)
- Security-hardened defaults for mobile deployment
- Rust release profile optimized for compact binaries
- Built-in safe skills bundle: `weather`, `summarize`, `github`, `healthcheck`, `session-logs`

## Performance Snapshot (Measured)

Measured on the same machine (`Darwin arm64`, 2026-02-15) with release builds:

| Metric | PhoneClaw |
| --- | ---: |
| Binary size (CLI release) | **~14 MB** |
| CLI startup (`status`, warm) | **0.00-0.03s** |
| Gateway idle RSS (~4s) | **~21.2 MB** |

Notes:
- PhoneClaw run included gateway subsystems (cron/heartbeat/metrics) enabled.

## Recent Performance Improvements (2026-02-17)

These are runtime improvements already implemented in PhoneClaw core:

- Provider reliability + retry backoff:
  - Added bounded retry/backoff for transient network, timeout, and 429 errors.
  - Reduces "typing but no final response" under unstable mobile network.
- Provider failover chain:
  - If the current provider fails, PhoneClaw now tries the next configured provider automatically.
  - Improves response success rate without manual app restart/reconfigure.
- Lightweight health diagnostics:
  - Added `phoneclaw doctor` for fast runtime checks (config/workspace/providers/channels/gateway).
  - Added `GET /api/monitor/health` with component-level health snapshot.
- Mobile-safe defaults:
  - Retry policy is bounded (small retry count, capped backoff), avoiding runaway latency and CPU wakeups.
  - Health snapshot and dedupe stats are exposed via lightweight JSON endpoint for low-overhead monitoring.

## Speed Tuning (Old Android)

Use this checklist to reduce lag and memory pressure on old phones.

### 1) Pick a fast/cheap model first

- Prefer lightweight models for daily chat:
  - `gpt-4o-mini`
  - `claude-3-5-haiku-latest`
  - `gemini-2.0-flash`
- Keep `max_tokens` moderate (`512-2048`) unless you really need long outputs.

### 2) Keep runtime concurrency low

In `~/.phoneclaw/config.json`, tune runtime for stability:

```json
{
  "runtime": {
    "adapter_max_inflight": 1,
    "adapter_retry_jitter_ms": 120,
    "ws_heartbeat_secs": 20,
    "health_window_minutes": 15,
    "dedupe_max_entries": 512
  }
}
```

Why this helps:
- Lower inflight work => lower RAM spikes.
- Smaller dedupe cache => less memory retained.
- Less frequent heartbeat => lower idle CPU wakeups.

### 3) Enable only channels you actually use

Every active adapter adds background work.
If you only need Telegram, keep only Telegram configured and remove unused channel configs.

### 4) Use Brave web search only when needed

`web_search` is network-heavy compared to plain chat.
For fast replies, avoid search unless user asks for latest/news/live information.

### 5) Prefer release builds

Always run release binaries:

```bash
cargo build --release -p phoneclaw-cli
./target/release/phoneclaw-cli gateway
```

### 6) Measure before/after

Use built-in diagnostics and monitor endpoints:

```bash
./target/release/phoneclaw-cli doctor
curl http://127.0.0.1:8080/api/monitor/health
curl http://127.0.0.1:8080/api/monitor/metrics
```

If monitor shows channel errors or degraded status, disable noisy channels first, then retest.

## Core Features

| Category | PhoneClaw |
| --- | --- |
| Runtime | Rust + Tokio/Axum |
| Android native app UI | Yes |
| Channels | Telegram, Discord, Slack, WhatsApp, Teams, Zalo, Google Chat |
| Provider config surface | OpenAI, OpenRouter, Anthropic, Google, Groq |
| Default tool posture | Security-hardened (reduced high-privilege tools) |

## Supported Usage Modes (Android only)

1. Android App (recommended)
2. Termux on Android (optional)

---

## 1) Android App (Recommended)

This is the easiest path for non-technical users.

### 1.1 Requirements

- Android Studio (recommended for building APK)
- Android SDK + NDK
- Rust toolchain
- `cargo-ndk`

Install `cargo-ndk` if missing:

```bash
cargo install cargo-ndk
```

### 1.2 Clone repository

```bash
git clone https://github.com/0xBoji/phoneclaw-rs.git
cd phoneclaw-rs
```

### 1.3 Build native libraries and APK

Build Rust Android libraries:

```bash
./build_android.sh
```

Build APK:

```bash
cd android
# Build with Android Studio or your Gradle setup
gradle assembleDebug
```

If Gradle says SDK path is missing, create `android/local.properties`:

```properties
sdk.dir=/absolute/path/to/Android/sdk
```

### 1.4 Install and configure app

After installing APK, open app and complete setup screens:

1. Workspace Creator
2. Provider & Secrets Manager
3. Channel Chat Setup
4. Skill Manifest Viewer & Permissions
5. Agent Control Dashboard
6. Resource & Log Monitor
7. Safety & Sandbox Toggles

Minimum required fields:
- Provider
- API key
- Model

Then open **Agent Control Dashboard** and press **Start Server**.

### 1.5 Verify local gateway

Use phone-local API endpoint:

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/api/status
curl http://127.0.0.1:8080/api/monitor/metrics
```

Send a message:

```bash
curl -X POST http://127.0.0.1:8080/api/message \
  -H "Content-Type: application/json" \
  -d '{"message":"Hello from Android local gateway"}'
```

---

## 2) Termux on Android (Optional)

Use this if you want CLI-first operation directly inside Termux.

### 2.1 Install dependencies

```bash
pkg update && pkg upgrade -y
pkg install -y git curl clang make pkg-config openssl
```

Install Rust:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### 2.2 Clone and build

```bash
git clone https://github.com/0xBoji/phoneclaw-rs.git
cd phoneclaw-rs
cargo build --release
```

### 2.3 Create config

```bash
mkdir -p ~/.phoneclaw
mkdir -p ~/phoneclaw-workspace

cat > ~/.phoneclaw/config.json << 'JSON'
{
  "workspace": "/data/data/com.termux/files/home/phoneclaw-workspace",
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

### 2.4 Run gateway

```bash
./target/release/phoneclaw-cli gateway
```

---

## 3) Configuration Notes

### 3.1 Provider examples

OpenRouter example:

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

### 3.2 Optional integrations

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

## 4) Security Defaults

- If gateway auth token is not set, gateway binds to localhost only.
- Tools are permission-gated by approved skills.
- Filesystem tool access is constrained to workspace.
- Web fetch includes SSRF checks for private/reserved IP ranges.

---

## 5) Troubleshooting

### `Failed to load config`
- Check `~/.phoneclaw/config.json` exists and valid JSON.

### Android build error: `SDK location not found`
- Set `ANDROID_HOME`, or create `android/local.properties` with `sdk.dir`.

### Termux runtime issues
- Ensure `source "$HOME/.cargo/env"`.
- Rebuild if needed: `cargo clean && cargo build --release`.

---

## 6) Project layout

- `crates/core` - shared types/config/security primitives
- `crates/agent` - agent loop, context, session handling
- `crates/tools` - tool system + sandbox controls
- `crates/providers` - LLM provider adapters
- `crates/server` - HTTP gateway
- `crates/cli` - CLI entrypoint
- `crates/mobile-jni` - JNI bridge for Android app
- `android/` - Android app project

---

## License

MIT
