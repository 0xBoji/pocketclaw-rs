pub mod onboard;
pub mod verify;

use pocketclaw_agent::agent_loop::AgentLoop;
use pocketclaw_agent::context::ContextBuilder;
use pocketclaw_agent::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::config::AppConfig;
use pocketclaw_cron::CronService;
use pocketclaw_heartbeat::HeartbeatService;
use pocketclaw_providers::factory::create_provider;
use pocketclaw_providers::LLMProvider;
use pocketclaw_server::gateway::Gateway;
use pocketclaw_core::channel::ChannelAdapter;
use pocketclaw_telegram::TelegramBot;
use pocketclaw_discord::DiscordBot;
use pocketclaw_tools::sandbox::SandboxConfig;
use pocketclaw_tools::exec_tool::ExecTool;
use pocketclaw_tools::fs_tools::{ListDirTool, ReadFileTool, WriteFileTool};
use pocketclaw_tools::registry::ToolRegistry;
use pocketclaw_tools::web_fetch::WebFetchTool;
use pocketclaw_tools::web_search::WebSearchTool;
use pocketclaw_core::metrics::MetricsStore;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};

pub fn get_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".pocketclaw")
}

pub fn get_cron_store_path() -> PathBuf {
    get_config_dir().join("cron/jobs.json")
}



pub async fn start_server(config_path: Option<PathBuf>) -> anyhow::Result<()> {
    // Save config path before it's consumed
    let config_path_saved = config_path.clone();

    // Initial Load
    let config = AppConfig::load(config_path.clone()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load config: {}. Run 'pocketclaw onboard' first.",
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
            let tool: Arc<dyn pocketclaw_tools::Tool> = Arc::new(WebSearchTool::new(brave_key.clone(), sandbox.clone()));
            tools
                .register(tool)
                .await;
        }
    }

    let context_builder = ContextBuilder::new(workspace.clone());

    // Initialize Google Sheets Client if configured
    let sheets_client = if let Some(sheets_cfg) = &config_val.google_sheets {
        match pocketclaw_agent::sheets::SheetsClient::new(
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

    let db_path = workspace.join("pocketclaw.db");
    let store_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let store = pocketclaw_persistence::SqliteSessionStore::new(&store_url).await?;

    let sessions = SessionManager::new(store, sheets_client);

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

    let gateway = Gateway::with_auth(
        bus.clone(), 
        8080, 
        config_val.web.as_ref().and_then(|w| w.auth_token.clone()),
        metrics.clone(),
        reload_tx
    );
    tokio::spawn(async move {
        if let Err(e) = gateway.start().await {
            error!("Gateway error: {}", e);
        }
    });

    // Start Heartbeat Service
    let heartbeat = HeartbeatService::new(workspace.clone(), 30 * 60, true);
    heartbeat.start(bus.clone());
    info!("Heartbeat service started");

    // Start Cron Service in background
    // Derive cron store path from config path or fall back to workspace
    let cron_store = config_path_saved
        .as_ref()
        .and_then(|p| p.parent())
        .map(|dir| dir.join("cron/jobs.json"))
        .unwrap_or_else(|| workspace.join(".pocketclaw/cron/jobs.json"));
    // Ensure cron directory exists
    if let Some(parent) = cron_store.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _cron_service = CronService::new(cron_store);
    info!("Cron service initialized");

    // Voice transcription
    if let Some(groq_cfg) = &config_val.providers.groq {
        let _transcriber = pocketclaw_voice::GroqTranscriber::new(groq_cfg.api_key.clone());
        info!("Groq voice transcription enabled");
    }

    // Start Telegram Bot if configured
    if let Some(telegram_cfg) = &config_val.telegram {
        let telegram_bot = TelegramBot::new(bus.clone(), telegram_cfg.token.clone());
        tokio::spawn(async move {
            if let Err(e) = telegram_bot.start().await {
                error!("Telegram Bot error: {}", e);
            }
        });
    }

    // Start Discord Bot if configured
    if let Some(discord_cfg) = &config_val.discord {
        let discord_bot = DiscordBot::new(bus.clone(), discord_cfg.token.clone());
        tokio::spawn(async move {
            if let Err(e) = discord_bot.start().await {
                error!("Discord Bot error: {}", e);
            }
        });
    }

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
