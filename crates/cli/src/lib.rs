pub mod onboard;
pub mod verify;

use pocketclaw_agent::agent_loop::AgentLoop;
use pocketclaw_agent::context::ContextBuilder;
use pocketclaw_agent::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::config::AppConfig;
use pocketclaw_cron::CronService;
use pocketclaw_heartbeat::HeartbeatService;
use pocketclaw_providers::anthropic::AnthropicProvider;
use pocketclaw_providers::google::GoogleProvider;
use pocketclaw_providers::openai::OpenAIProvider;
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
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

pub fn get_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".pocketclaw")
}

pub fn get_cron_store_path() -> PathBuf {
    get_config_dir().join("cron/jobs.json")
}

pub fn create_provider(config: &AppConfig) -> anyhow::Result<Arc<dyn LLMProvider>> {
    if let Some(openai_cfg) = &config.providers.openai {
        Ok(Arc::new(OpenAIProvider::new(
            openai_cfg.api_key.clone(),
            openai_cfg.api_base.clone(),
        )))
    } else if let Some(openrouter_cfg) = &config.providers.openrouter {
        Ok(Arc::new(OpenAIProvider::new(
            openrouter_cfg.api_key.clone(),
            openrouter_cfg.api_base.clone(),
        )))
    } else if let Some(anthropic_cfg) = &config.providers.anthropic {
        Ok(Arc::new(AnthropicProvider::new(
            anthropic_cfg.api_key.clone(),
        )))
    } else if let Some(google_cfg) = &config.providers.google {
        Ok(Arc::new(GoogleProvider::new(
            google_cfg.api_key.clone(),
            google_cfg.model.clone(),
        )))
    } else {
        anyhow::bail!("No LLM provider configured. Run 'pocketclaw onboard' to set one up.");
    }
}

pub async fn start_server(config_path: Option<PathBuf>) -> anyhow::Result<()> {
    // Save config path before it's consumed
    let config_path_saved = config_path.clone();

    // Load config
    let config = AppConfig::load(config_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load config: {}. Run 'pocketclaw onboard' first.",
            e
        )
    })?;
    let workspace = config.workspace.clone();

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
    let bus = Arc::new(MessageBus::new(100));
    let provider: Arc<dyn LLMProvider> = create_provider(&config)?;

    let tools = ToolRegistry::new();
    tools.register(Arc::new(ExecTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(ReadFileTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(WriteFileTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(ListDirTool::new(sandbox.clone()))).await;
    tools.register(Arc::new(WebFetchTool::new())).await;

    if let Some(web_cfg) = &config.web {
        if let Some(brave_key) = &web_cfg.brave_key {
            tools
                .register(Arc::new(WebSearchTool::new(brave_key.clone())))
                .await;
        }
    }

    let context_builder = ContextBuilder::new(workspace.clone());

    // Initialize Google Sheets Client if configured
    let sheets_client = if let Some(sheets_cfg) = &config.google_sheets {
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

    let sessions = SessionManager::new(workspace.clone(), sheets_client);

    let agent = AgentLoop::new(
        bus.clone(),
        config.clone(),
        provider,
        tools,
        context_builder,
        sessions,
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

    let gateway = Gateway::new(bus.clone(), 8080);
    tokio::spawn(async move {
        if let Err(e) = gateway.start().await {
            error!("Gateway error: {}", e);
        }
    });

    // Start Heartbeat Service
    let heartbeat = HeartbeatService::new(workspace.clone(), 30 * 60, true);
    heartbeat.start();
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
    if let Some(groq_cfg) = &config.providers.groq {
        let _transcriber = pocketclaw_voice::GroqTranscriber::new(groq_cfg.api_key.clone());
        info!("Groq voice transcription enabled");
    }

    // Start Telegram Bot if configured
    if let Some(telegram_cfg) = &config.telegram {
        let telegram_bot = TelegramBot::new(bus.clone(), telegram_cfg.token.clone());
        tokio::spawn(async move {
            if let Err(e) = telegram_bot.start().await {
                error!("Telegram Bot error: {}", e);
            }
        });
    }

    // Start Discord Bot if configured
    if let Some(discord_cfg) = &config.discord {
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
    
    // Run the agent loop (this blocks)
    agent.run().await;

    Ok(())
}
