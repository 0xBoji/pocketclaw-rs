use clap::{Parser, Subcommand};
use pocketclaw_agent::agent_loop::AgentLoop;
use pocketclaw_agent::context::ContextBuilder;
use pocketclaw_agent::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::channel::{
    is_native_channel_supported, CHANNEL_GOOGLE_CHAT, CHANNEL_IMESSAGE, CHANNEL_MATRIX,
    CHANNEL_SIGNAL, CHANNEL_TEAMS, CHANNEL_WEBCHAT, CHANNEL_ZALO,
};
use pocketclaw_core::config::AppConfig;
use pocketclaw_core::types::{Message, Role};
use pocketclaw_cron::{CronSchedule, CronService};
use pocketclaw_heartbeat::HeartbeatService;
use pocketclaw_providers::factory::create_provider;
use pocketclaw_providers::LLMProvider;
use pocketclaw_server::gateway::{Gateway, GatewayRuntimeConfig};
use pocketclaw_core::channel::ChannelAdapter;
use pocketclaw_telegram::TelegramBot;
use pocketclaw_discord::DiscordBot;
use pocketclaw_slack::SlackAdapter;
use pocketclaw_teams::TeamsAdapter;
use pocketclaw_whatsapp::WhatsAppAdapter;
use pocketclaw_zalo::ZaloAdapter;
use pocketclaw_googlechat::GoogleChatAdapter;
use pocketclaw_tools::exec_tool::ExecTool;
use pocketclaw_tools::fs_tools::{ListDirTool, ReadFileTool, WriteFileTool};
use pocketclaw_tools::registry::ToolRegistry;
use pocketclaw_tools::platform_tools::{ChannelHealthTool, DatetimeNowTool, MetricsSnapshotTool};
use pocketclaw_tools::sessions_tools::{SessionsHistoryTool, SessionsListTool, SessionsSendTool};
use pocketclaw_tools::sandbox::SandboxConfig;
use pocketclaw_tools::web_fetch::WebFetchTool;
use pocketclaw_tools::web_search::WebSearchTool;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use tracing_appender;
use tokio::sync::{mpsc, RwLock};
use pocketclaw_core::metrics::MetricsStore;

const VERSION: &str = "0.1.0";

#[derive(Parser)]
#[command(name = "pocketclaw")]
#[command(version = VERSION)]
#[command(about = "🦞 Ultra-lightweight personal AI assistant in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the agent in interactive mode
    Agent {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Start the gateway server (with Telegram/Discord bots)
    Gateway {
        /// Use mock Android bridge for testing on desktop
        #[arg(long)]
        mock_android: bool,
    },
    /// Run the onboarding wizard
    Onboard,
    /// Show pocketclaw status
    Status,
    /// Manage scheduled tasks
    Cron {
        #[command(subcommand)]
        action: CronActions,
    },
    /// Manage skills (install, list, remove)
    Skills {
        #[command(subcommand)]
        action: SkillsActions,
    },
}

#[derive(Subcommand)]
enum CronActions {
    /// List all scheduled jobs
    List,
    /// Add a new scheduled job
    Add {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        message: String,
        /// Run every N seconds
        #[arg(short, long)]
        every: Option<i64>,
        /// Deliver response to channel
        #[arg(short, long, default_value_t = false)]
        deliver: bool,
        /// Channel for delivery
        #[arg(long)]
        channel: Option<String>,
        /// Recipient for delivery
        #[arg(long)]
        to: Option<String>,
    },
    /// Remove a job by ID
    Remove {
        /// Job ID to remove
        id: String,
    },
    /// Enable a job by ID
    Enable {
        /// Job ID to enable
        id: String,
    },
    /// Disable a job by ID
    Disable {
        /// Job ID to disable
        id: String,
    },
}

#[derive(Subcommand)]
enum SkillsActions {
    /// List installed skills
    List,
    /// Show skill details
    Show {
        /// Skill name
        name: String,
    },
    /// Approve a skill for use
    Approve {
        /// Skill name
        name: String,
    },
    /// Revoke approval for a skill
    Revoke {
        /// Skill name
        name: String,
    },
}

mod onboard;

fn get_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".pocketclaw")
}

fn get_cron_store_path() -> PathBuf {
    get_config_dir().join("cron/jobs.json")
}

fn log_channel_readiness(config: &AppConfig) {
    for channel in config.configured_channels() {
        if is_native_channel_supported(channel) {
            info!("Channel '{}' is configured and has a native adapter", channel);
        } else {
            info!(
                "Channel '{}' is configured but adapter is pending; config is preserved for rollout",
                channel
            );
        }
    }
}

fn spawn_channel_adapters(bus: Arc<MessageBus>, config: &AppConfig) {
    let adapter_max_inflight = runtime_adapter_max_inflight(config);
    let adapter_retry_jitter_ms = runtime_adapter_retry_jitter_ms(config);

    if let Some(whatsapp_cfg) = &config.whatsapp {
        let whatsapp = WhatsAppAdapter::new(
            bus.clone(),
            whatsapp_cfg.token.clone(),
            whatsapp_cfg.api_base.clone(),
            whatsapp_cfg.phone_number_id.clone(),
            whatsapp_cfg.default_to.clone(),
            adapter_max_inflight,
            adapter_retry_jitter_ms,
        );
        tokio::spawn(async move {
            if let Err(e) = whatsapp.start().await {
                error!("WhatsApp adapter error: {}", e);
            }
        });
    }

    if let Some(telegram_cfg) = &config.telegram {
        let telegram_bot = TelegramBot::new(bus.clone(), telegram_cfg.token.clone());
        tokio::spawn(async move {
            if let Err(e) = telegram_bot.start().await {
                error!("Telegram Bot error: {}", e);
            }
        });
    }

    if let Some(discord_cfg) = &config.discord {
        let discord_bot = DiscordBot::new(bus.clone(), discord_cfg.token.clone());
        tokio::spawn(async move {
            if let Err(e) = discord_bot.start().await {
                error!("Discord Bot error: {}", e);
            }
        });
    }

    if let Some(slack_cfg) = &config.slack {
        let slack = SlackAdapter::new(
            bus.clone(),
            slack_cfg.bot_token.clone(),
            slack_cfg.default_channel.clone(),
            adapter_max_inflight,
            adapter_retry_jitter_ms,
        );
        tokio::spawn(async move {
            if let Err(e) = slack.start().await {
                error!("Slack adapter error: {}", e);
            }
        });
    }

    if config.signal.is_some() {
        info!("{} is configured (adapter rollout pending)", CHANNEL_SIGNAL);
    }
    if config.imessage.as_ref().is_some_and(|cfg| cfg.enabled) {
        info!("{} is configured (adapter rollout pending)", CHANNEL_IMESSAGE);
    }
    if let Some(teams_cfg) = &config.teams {
        if let Some(webhook_url) = teams_cfg.webhook_url.clone() {
            let teams = TeamsAdapter::new(
                bus.clone(),
                webhook_url,
                adapter_max_inflight,
                adapter_retry_jitter_ms,
            );
            tokio::spawn(async move {
                if let Err(e) = teams.start().await {
                    error!("Teams adapter error: {}", e);
                }
            });
        } else {
            info!("{} is configured but missing webhook_url", CHANNEL_TEAMS);
        }
    }
    if config.matrix.is_some() {
        info!("{} is configured (adapter rollout pending)", CHANNEL_MATRIX);
    }
    if let Some(zalo_cfg) = &config.zalo {
        if let Some(webhook_url) = zalo_cfg.webhook_url.clone() {
            let zalo = ZaloAdapter::new(
                bus.clone(),
                zalo_cfg.token.clone(),
                webhook_url,
                zalo_cfg.default_to.clone(),
                adapter_max_inflight,
                adapter_retry_jitter_ms,
            );
            tokio::spawn(async move {
                if let Err(e) = zalo.start().await {
                    error!("Zalo adapter error: {}", e);
                }
            });
        } else {
            info!("{} is configured but missing webhook_url", CHANNEL_ZALO);
        }
    }
    if let Some(google_chat_cfg) = &config.google_chat {
        let google_chat = GoogleChatAdapter::new(
            bus.clone(),
            google_chat_cfg.webhook_url.clone(),
            adapter_max_inflight,
            adapter_retry_jitter_ms,
        );
        tokio::spawn(async move {
            if let Err(e) = google_chat.start().await {
                error!("Google Chat adapter error: {}", e);
            }
        });
    }
    if config.webchat.as_ref().is_some_and(|cfg| cfg.enabled) {
        info!(
            "{} is configured (HTTP gateway path serves as base transport)",
            CHANNEL_WEBCHAT
        );
    }
    if config.google_chat.is_some() {
        info!("{} is configured", CHANNEL_GOOGLE_CHAT);
    }
}

fn runtime_adapter_max_inflight(config: &AppConfig) -> usize {
    config
        .runtime
        .as_ref()
        .and_then(|r| r.adapter_max_inflight)
        .unwrap_or(1)
        .clamp(1, 8)
}

fn runtime_adapter_retry_jitter_ms(config: &AppConfig) -> u64 {
    config
        .runtime
        .as_ref()
        .and_then(|r| r.adapter_retry_jitter_ms)
        .unwrap_or(150)
        .clamp(0, 2000)
}

fn runtime_gateway_config(config: &AppConfig) -> GatewayRuntimeConfig {
    GatewayRuntimeConfig {
        ws_heartbeat_secs: config
            .runtime
            .as_ref()
            .and_then(|r| r.ws_heartbeat_secs)
            .unwrap_or(15)
            .clamp(3, 120),
        health_window_minutes: config
            .runtime
            .as_ref()
            .and_then(|r| r.health_window_minutes)
            .map(|v| v as usize)
            .unwrap_or(60)
            .clamp(5, 60),
        dedupe_max_entries: config
            .runtime
            .as_ref()
            .and_then(|r| r.dedupe_max_entries)
            .unwrap_or(2048)
            .clamp(128, 20_000),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    // Initialize logging
    let (non_blocking, _guard) = tracing_appender::non_blocking(tracing_appender::rolling::daily(
        get_config_dir().join("logs"),
        "audit.jsonl",
    ));

    let audit_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .with_level(false)
        .with_file(false)
        .with_line_number(false)
        .without_time() // Timestamp is in JSON
        .with_filter(tracing_subscriber::filter::Targets::new().with_target("audit", Level::INFO));

    let stdout_filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(Level::INFO.into());
        // We want to exclude "audit" target from stdout, but EnvFilter doesn't support exclusion easily.
        // We'll rely on the fact that we can wrap stdout layer with a filter_fn.

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(stdout_filter)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target() != "audit"
        }));

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(audit_layer)
        .init();

    let cli = Cli::parse();

    match &cli.command {
        // === Commands that DON'T require config ===
        Some(Commands::Onboard) => {
            if let Err(e) = onboard::run_onboarding() {
                error!("Onboarding failed: {}", e);
            }
            return Ok(());
        }
        Some(Commands::Status) => {
            run_status();
            return Ok(());
        }
        Some(Commands::Cron { action }) => {
            run_cron(action);
            return Ok(());
        }
        _ => {}
    }

    // === Commands that DO require config ===
    let config = AppConfig::load(None).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load config: {}. Run 'pocketclaw onboard' first.",
            e
        )
    })?;
    let config = Arc::new(RwLock::new(config));
    let metrics = MetricsStore::new();
    let (reload_tx, _reload_rx) = mpsc::channel(1); // main.rs doesn't handle reloads yet, just needs it for Gateway

    let config_val = config.read().await.clone();
    let workspace = config_val.workspace.clone();

    // ensure workspace exists
    tokio::fs::create_dir_all(&workspace).await?;

    // Create Sandbox Config
    let sandbox = SandboxConfig {
        workspace_path: workspace.clone(),
        exec_timeout_secs: 30,
        max_output_bytes: 64 * 1024,
        exec_enabled: true,
        network_allowlist: Vec::new(),
        ..Default::default()
    };

    // Create Components
    let bus = Arc::new(MessageBus::new(100).with_metrics(metrics.clone()));
    let provider: Arc<dyn LLMProvider> = create_provider(&config_val)?;

    let tools = ToolRegistry::new();
    tools.register(Arc::new(ExecTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(ReadFileTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(WriteFileTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(ListDirTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(WebFetchTool::new(sandbox.clone()))).await;

    // Register Mock Android Tools if requested
    if let Some(Commands::Gateway { mock_android: true }) = &cli.command {
        info!("Using Mock Android Bridge");
        let bridge = Arc::new(MockAndroidBridge);
        tools.register(Arc::new(pocketclaw_tools::android_tools::AndroidActionTool::new(bridge.clone()))).await;
        tools.register(Arc::new(pocketclaw_tools::android_tools::AndroidScreenTool::new(bridge.clone()))).await;
        info!("Mock Android Tools registered");
    }

    if let Some(web_cfg) = &config_val.web {
        if let Some(brave_key) = &web_cfg.brave_key {
            let tool: Arc<dyn pocketclaw_tools::Tool> = Arc::new(WebSearchTool::new(brave_key.clone(), sandbox.clone()));
            tools
                .register(tool)
                .await;
        }
    }

    let db_path = workspace.join("pocketclaw.db");
    let store_url = format!("sqlite://{}?mode=rwc", db_path.display()); // rwc = read write create
    let store = pocketclaw_persistence::SqliteSessionStore::new(&store_url).await?;

    let sheets_client = if let Some(sheets_cfg) = &config_val.google_sheets {
        match pocketclaw_agent::sheets::SheetsClient::new(
            sheets_cfg.service_account_json.clone(),
            sheets_cfg.spreadsheet_id.clone(),
        ).await {
            Ok(client) => Some(client),
            Err(e) => {
                error!("Failed to initialize SheetsClient: {}", e);
                None
            }
        }
    } else {
        None
    };

    let context_builder = ContextBuilder::new(workspace.clone());
    let session_store = store.clone();
    let sessions = SessionManager::new(store, sheets_client);

    tools
        .register(Arc::new(SessionsListTool::new(session_store.clone())))
        .await;
    tools
        .register(Arc::new(SessionsHistoryTool::new(session_store.clone())))
        .await;
    tools
        .register(Arc::new(SessionsSendTool::new(bus.clone())))
        .await;
    tools
        .register(Arc::new(ChannelHealthTool::new(
            "http://127.0.0.1:8080".to_string(),
            config_val.web.as_ref().and_then(|w| w.auth_token.clone()),
        )))
        .await;
    tools
        .register(Arc::new(MetricsSnapshotTool::new(
            "http://127.0.0.1:8080".to_string(),
            config_val.web.as_ref().and_then(|w| w.auth_token.clone()),
        )))
        .await;
    tools.register(Arc::new(DatetimeNowTool::new())).await;

    // Start Cron Service in background
    let cron_store_path = get_cron_store_path();
    if let Some(parent) = cron_store_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let cron_service = Arc::new(CronService::new(cron_store_path));
    let _cron_loop_handle = cron_service.clone().start_loop(bus.clone());
    info!("Cron service initialized and loop started");

    tools.register(Arc::new(pocketclaw_tools::cron_tools::CronTool::new(cron_service.clone()))).await;

    let agent = AgentLoop::new(
        bus.clone(),
        config.clone(),
        provider,
        tools,
        context_builder,
        sessions,
        metrics.clone(),
    );

    // Subscribe to Bus for logging
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        loop {
            if let Ok(event) = rx.recv().await {
                match event {
                    Event::OutboundMessage(msg) => {
                        println!("\n🦞 PocketClaw: {}\n", msg.content);
                    }
                    _ => {}
                }
            }
        }
    });

    match &cli.command {
        Some(Commands::Gateway { mock_android }) => {

            let gateway = Gateway::with_auth(
                bus.clone(), 
                8080, 
                config_val.web.as_ref().and_then(|w| w.auth_token.clone()),
                metrics.clone(),
                reload_tx,
                session_store.clone(),
                config_val
                    .whatsapp
                    .as_ref()
                    .and_then(|w| w.verify_token.clone()),
                config_val
                    .whatsapp
                    .as_ref()
                    .and_then(|w| w.app_secret.clone()),
                config_val
                    .slack
                    .as_ref()
                    .and_then(|s| s.signing_secret.clone()),
                config_val
                    .configured_channels()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                runtime_gateway_config(&config_val),
            );
            tokio::spawn(async move {
                if let Err(e) = gateway.start().await {
                    error!("Gateway error: {}", e);
                }
            });
            log_channel_readiness(&config_val);

            // Start Heartbeat Service
            let heartbeat = HeartbeatService::new(workspace.clone(), 30 * 60, true);
            heartbeat.start(bus.clone());
            info!("Heartbeat service started");

            // Voice transcription
            if let Some(groq_cfg) = &config_val.providers.groq {
                let _transcriber = pocketclaw_voice::GroqTranscriber::new(groq_cfg.api_key.clone());
                info!("Groq voice transcription enabled");
            }

            spawn_channel_adapters(bus.clone(), &config_val);

            println!("🦞 Gateway started on 0.0.0.0:8080");
            if *mock_android {
                println!("📱 Mock Android Mode: ENABLED");
            }
            println!("✓ Heartbeat service started");
            println!("✓ Cron service initialized");
            println!("Press Ctrl+C to stop");
            agent.run().await;
        }
        Some(Commands::Agent { message }) => {
            tokio::spawn(async move {
                agent.run().await;
            });

            if let Some(msg) = message {
                let inbound = Message::new("cli", "default", Role::User, msg);
                bus.publish(Event::InboundMessage(inbound))?;

                // Wait a bit for response
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            } else {
                println!("🦞 Interactive mode not fully implemented yet. Use -m to send a message.");
            }
        }
        Some(Commands::Skills { action }) => {
            run_skills(action, &workspace);
        }
        _ => {
            println!("🦞 pocketclaw v{}", VERSION);
            println!("Use --help for usage.");
        }
    }

    Ok(())
}

use pocketclaw_tools::android_tools::AndroidBridge;
use async_trait::async_trait;

struct MockAndroidBridge;

#[async_trait]
impl AndroidBridge for MockAndroidBridge {
    async fn click(&self, x: f32, y: f32) -> Result<bool, String> {
        info!("MOCK: Click at ({}, {})", x, y);
        Ok(true)
    }
    async fn scroll(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> Result<bool, String> {
        info!("MOCK: Scroll from ({}, {}) to ({}, {})", x1, y1, x2, y2);
        Ok(true)
    }
    async fn back(&self) -> Result<bool, String> {
        info!("MOCK: Back");
        Ok(true)
    }
    async fn home(&self) -> Result<bool, String> {
        info!("MOCK: Home");
        Ok(true)
    }
    async fn input_text(&self, text: String) -> Result<bool, String> {
        info!("MOCK: Input text '{}'", text);
        Ok(true)
    }
    async fn dump_hierarchy(&self) -> Result<String, String> {
        info!("MOCK: Dump Hierarchy");
        Ok(r#"<node class="android.widget.FrameLayout" bounds="[0,0][1080,2400]">
  <node class="android.widget.Button" text="Search" id="search_button" bounds="[100,100][200,200]" clickable="true" />
  <node class="android.widget.EditText" text="" id="search_input" bounds="[200,100][800,200]" editable="true" />
  <node class="android.widget.TextView" text="Welcome to Mock Android" bounds="[100,300][900,400]" />
</node>"#.to_string())
    }
    async fn screenshot(&self) -> Result<Vec<u8>, String> {
        info!("MOCK: Screenshot");
        // 1x1 Transparent PNG
        Ok(vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 0, 0, 2, 0, 1, 244, 113, 100, 31, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130
        ])
    }
}



fn run_status() {
    let config_path = get_config_dir().join("config.json");

    println!("🦞 pocketclaw Status\n");

    if config_path.exists() {
        println!("Config: {} ✓", config_path.display());
    } else {
        println!("Config: {} ✗ (run 'pocketclaw onboard')", config_path.display());
        return;
    }

    match AppConfig::load(None) {
        Ok(config) => {
            let workspace = config.workspace.clone();
            if workspace.exists() {
                println!("Workspace: {} ✓", workspace.display());
            } else {
                println!("Workspace: {} ✗", workspace.display());
            }

            println!("Model: {}", config.agents.default.model);

            let check = |name: &str, has: bool| {
                if has {
                    println!("{}: ✓", name);
                } else {
                    println!("{}: not set", name);
                }
            };

            check("OpenAI API", config.providers.openai.is_some());
            check("OpenRouter API", config.providers.openrouter.is_some());
            check("Anthropic API", config.providers.anthropic.is_some());
            check("Google Gemini API", config.providers.google.is_some());
            check("Groq API", config.providers.groq.is_some());
            check("WhatsApp", config.whatsapp.is_some());
            check("Telegram Bot", config.telegram.is_some());
            check("Slack Bot", config.slack.is_some());
            check("Discord Bot", config.discord.is_some());
            check("Signal", config.signal.is_some());
            check(
                "iMessage",
                config.imessage.as_ref().is_some_and(|cfg| cfg.enabled),
            );
            check("Teams", config.teams.is_some());
            check("Matrix", config.matrix.is_some());
            check("Zalo", config.zalo.is_some());
            check(
                "WebChat",
                config.webchat.as_ref().is_some_and(|cfg| cfg.enabled),
            );
            check(
                "Web Search",
                config
                    .web
                    .as_ref()
                    .and_then(|w| w.brave_key.as_ref())
                    .is_some(),
            );
        }
        Err(e) => {
            println!("Error loading config: {}", e);
        }
    }
}

fn run_cron(action: &CronActions) {
    let store_path = get_cron_store_path();
    let service = CronService::new(store_path);

    match action {
        CronActions::List => {
            let jobs = service.list_jobs(true);
            if jobs.is_empty() {
                println!("No scheduled jobs.");
                return;
            }

            println!("\nScheduled Jobs:");
            println!("----------------");
            for job in &jobs {
                let schedule = match job.schedule.kind.as_str() {
                    "every" => {
                        if let Some(ms) = job.schedule.every_ms {
                            format!("every {}s", ms / 1000)
                        } else {
                            "every ?".to_string()
                        }
                    }
                    "cron" => job
                        .schedule
                        .expr
                        .clone()
                        .unwrap_or_else(|| "?".to_string()),
                    _ => "one-time".to_string(),
                };

                let status = if job.enabled { "enabled" } else { "disabled" };

                println!("  {} ({})", job.name, job.id);
                println!("    Schedule: {}", schedule);
                println!("    Status: {}", status);
            }
        }
        CronActions::Add {
            name,
            message,
            every,
            deliver,
            channel,
            to,
        } => {
            let schedule = if let Some(secs) = every {
                CronSchedule {
                    kind: "every".to_string(),
                    at_ms: None,
                    every_ms: Some(secs * 1000),
                    expr: None,
                }
            } else {
                println!("Error: --every is required");
                return;
            };

            match service.add_job(
                name.clone(),
                schedule,
                message.clone(),
                *deliver,
                channel.clone(),
                to.clone(),
            ) {
                Ok(job) => println!("✓ Added job '{}' ({})", job.name, job.id),
                Err(e) => println!("Error adding job: {}", e),
            }
        }
        CronActions::Remove { id } => {
            if service.remove_job(id) {
                println!("✓ Removed job {}", id);
            } else {
                println!("✗ Job {} not found", id);
            }
        }
        CronActions::Enable { id } => match service.enable_job(id, true) {
            Some(job) => println!("✓ Job '{}' enabled", job.name),
            None => println!("✗ Job {} not found", id),
        },
        CronActions::Disable { id } => match service.enable_job(id, false) {
            Some(job) => println!("✓ Job '{}' disabled", job.name),
            None => println!("✗ Job {} not found", id),
        },
    }
}

fn run_skills(action: &SkillsActions, workspace: &std::path::Path) {
    let skills_dir = workspace.join("skills");

    match action {
        SkillsActions::List => {
            if !skills_dir.exists() {
                println!("No skills directory found.");
                return;
            }

            let entries = match std::fs::read_dir(&skills_dir) {
                Ok(e) => e,
                Err(e) => {
                    println!("Error reading skills directory: {}", e);
                    return;
                }
            };

            println!("\nInstalled Skills:");
            println!("------------------");
            let mut count = 0;
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let skill_name = entry.file_name().to_string_lossy().to_string();
                    let skill_file = entry.path().join("SKILL.md");
                    let status = if skill_file.exists() { "✓" } else { "✗" };
                    println!("  {} {}", status, skill_name);
                    count += 1;
                }
            }

            if count == 0 {
                println!("  No skills installed.");
            }
        }
        SkillsActions::Show { name } => {
            let skill_path = skills_dir.join(name).join("SKILL.md");
            // Check manifest first
            let manifest_path = skills_dir.join(name).join("skill.toml");
            
            if manifest_path.exists() {
                 match std::fs::read_to_string(&manifest_path) {
                    Ok(content) => {
                        println!("\n📦 Skill: {} (Manifest)", name);
                        println!("----------------------");
                        println!("{}", content);
                    }
                    Err(_) => println!("✗ Failed to read manifest for '{}'", name),
                }
            } else if skill_path.exists() {
                match std::fs::read_to_string(&skill_path) {
                    Ok(content) => {
                        println!("\n📦 Skill: {} (Legacy)", name);
                        println!("----------------------");
                        println!("{}", content);
                    }
                    Err(_) => println!("✗ Skill '{}' not found", name),
                }
            } else {
                println!("✗ Skill '{}' not found", name);
            }
        }
        SkillsActions::Approve { name } => {
            use pocketclaw_core::permissions::ApprovedSkills;
            let mut approved = ApprovedSkills::load(&ApprovedSkills::default_path());
            
            // Verify skill exists?
            // Yes, we should check if it exists in skills_dir
            if !skills_dir.join(name).exists() {
                println!("⚠️ Warning: Skill '{}' not found in workspace.", name);
                println!("Approving anyway (maybe it will be installed later)...");
            }
            
            approved.approve(name.clone());
            if let Err(e) = approved.save(&ApprovedSkills::default_path()) {
                println!("✗ Failed to save approved skills: {}", e);
            } else {
                println!("✓ Skill '{}' approved.", name);
            }
        }
        SkillsActions::Revoke { name } => {
            use pocketclaw_core::permissions::ApprovedSkills;
            let mut approved = ApprovedSkills::load(&ApprovedSkills::default_path());
            
            approved.revoke(name);
            if let Err(e) = approved.save(&ApprovedSkills::default_path()) {
                println!("✗ Failed to save approved skills: {}", e);
            } else {
                println!("✓ Approval revoked for '{}'.", name);
            }
        }
    }
}
