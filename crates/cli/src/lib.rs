pub mod onboard;
pub mod verify;

use phoneclaw_agent::agent_loop::AgentLoop;
use phoneclaw_agent::context::ContextBuilder;
use phoneclaw_agent::session::SessionManager;
use phoneclaw_core::bus::{Event, MessageBus};
use phoneclaw_core::channel::{
    is_native_channel_supported, CHANNEL_GOOGLE_CHAT, CHANNEL_IMESSAGE, CHANNEL_MATRIX,
    CHANNEL_SIGNAL, CHANNEL_TEAMS, CHANNEL_WEBCHAT, CHANNEL_ZALO,
};
use phoneclaw_core::config::AppConfig;
use phoneclaw_cron::CronService;
use phoneclaw_heartbeat::HeartbeatService;
use phoneclaw_providers::factory::create_provider;
use phoneclaw_providers::LLMProvider;
use phoneclaw_server::gateway::{Gateway, GatewayRuntimeConfig};
use phoneclaw_core::channel::ChannelAdapter;
use phoneclaw_telegram::TelegramBot;
use phoneclaw_discord::DiscordBot;
use phoneclaw_slack::SlackAdapter;
use phoneclaw_teams::TeamsAdapter;
use phoneclaw_whatsapp::WhatsAppAdapter;
use phoneclaw_zalo::ZaloAdapter;
use phoneclaw_googlechat::GoogleChatAdapter;
use phoneclaw_tools::sandbox::SandboxConfig;
use phoneclaw_tools::exec_tool::ExecTool;
use phoneclaw_tools::fs_tools::{ListDirTool, ReadFileTool, WriteFileTool};
use phoneclaw_tools::registry::ToolRegistry;
use phoneclaw_tools::platform_tools::{ChannelHealthTool, DatetimeNowTool, MetricsSnapshotTool};
use phoneclaw_tools::sessions_tools::{SessionsHistoryTool, SessionsListTool, SessionsSendTool};
use phoneclaw_tools::web_fetch::WebFetchTool;
use phoneclaw_tools::web_search::WebSearchTool;
use phoneclaw_core::metrics::MetricsStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

pub fn get_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".phoneclaw")
}

pub fn get_cron_store_path() -> PathBuf {
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



use phoneclaw_tools::android_tools::{AndroidBridge, AndroidActionTool, AndroidScreenTool};

pub async fn start_server(config_path: Option<PathBuf>, android_bridge: Option<Arc<dyn AndroidBridge>>) -> anyhow::Result<()> {
    // Save config path before it's consumed
    let config_path_saved = config_path.clone();

    // Initial Load
    let config = AppConfig::load(config_path.clone()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load config: {}. Run 'phoneclaw onboard' first.",
            e
        )
    })?;
    let config = Arc::new(RwLock::new(config));
    let metrics = MetricsStore::new();
    let (reload_tx, mut reload_rx) = mpsc::channel(1);

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

    if let Some(web_cfg) = &config_val.web {
        if let Some(brave_key) = &web_cfg.brave_key {
            let tool: Arc<dyn phoneclaw_tools::Tool> = Arc::new(WebSearchTool::new(brave_key.clone(), sandbox.clone()));
            tools
                .register(tool)
                .await;
        }
    }

    if let Some(bridge) = android_bridge {
        tools.register(Arc::new(AndroidActionTool::new(bridge.clone()))).await;
        tools.register(Arc::new(AndroidScreenTool::new(bridge.clone()))).await;
        info!("Android Action & Screen Tools registered");
    }

    let context_builder = ContextBuilder::new(workspace.clone());

    // Initialize Google Sheets Client if configured
    let sheets_client = if let Some(sheets_cfg) = &config_val.google_sheets {
        match phoneclaw_agent::sheets::SheetsClient::new(
            sheets_cfg.service_account_json.clone(),
            sheets_cfg.spreadsheet_id.clone(),
        )
        .await
        {
            Ok(client) => {
                info!("Google Sheets memory enabled");
                Some(client)
            }
            Err(e) => {
                error!("Failed to initialize Google Sheets client: {}", e);
                None
            }
        }
    } else {
        None
    };

    let db_path = workspace.join("phoneclaw.db");
    let store_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let store = phoneclaw_persistence::SqliteSessionStore::new(&store_url).await?;
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
    let cron_store_path = config_path_saved
        .as_ref()
        .and_then(|p| p.parent())
        .map(|dir| dir.join("cron/jobs.json"))
        .unwrap_or_else(|| workspace.join(".phoneclaw/cron/jobs.json"));
    if let Some(parent) = cron_store_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let cron_service = Arc::new(CronService::new(cron_store_path));
    let _cron_loop_handle = cron_service.clone().start_loop(bus.clone());
    info!("Cron service initialized and loop started");

    tools.register(Arc::new(phoneclaw_tools::cron_tools::CronTool::new(cron_service.clone()))).await;

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
                        println!("\n🦞 PhoneClaw: {}\n", msg.content);
                    }
                    _ => {}
                }
            }
        }
    });

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
        let _transcriber = phoneclaw_voice::GroqTranscriber::new(groq_cfg.api_key.clone());
        info!("Groq voice transcription enabled");
    }

    spawn_channel_adapters(bus.clone(), &config_val);

    println!("🦞 Gateway started on 0.0.0.0:8080");
    println!("✓ Heartbeat service started");
    println!("✓ Cron service initialized");
    println!("Press Ctrl+C to stop");

    // Start Reload Monitor
    let config_for_reload = config.clone();
    let config_path_for_reload = config_path_saved.clone();
    tokio::spawn(async move {
        while let Some(_) = reload_rx.recv().await {
            info!("Reloading configuration...");
            match AppConfig::load(config_path_for_reload.clone()) {
                Ok(new_config) => {
                    let mut lock = config_for_reload.write().await;
                    *lock = new_config;
                    info!("Configuration reloaded successfully");
                    // Note: In a full impl, we would also need to refresh Providers/Bots
                    // but since they use Arc<RwLock<Config>> or are stateless, it works mostly.
                }
                Err(e) => error!("Failed to reload config: {}", e),
            }
        }
    });
    
    // Run the agent loop (this blocks)
    agent.run().await;

    Ok(())
}
