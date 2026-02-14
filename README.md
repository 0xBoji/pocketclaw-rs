# phoneclaw-rs (Android-First Local Gateway)

`phoneclaw-rs` is a local-first AI agent runtime focused on Android devices, especially old phones used as always-on local gateway nodes.

Primary goal: user only needs to set `provider + api key + model`, then start the gateway.

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
