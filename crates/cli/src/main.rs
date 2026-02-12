use clap::{Parser, Subcommand};
use pocketclaw_agent::agent_loop::AgentLoop;
use pocketclaw_agent::context::ContextBuilder;
use pocketclaw_agent::session::SessionManager;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::config::AppConfig;
use pocketclaw_core::types::{Message, Role};
use pocketclaw_cron::{CronSchedule, CronService};
use pocketclaw_heartbeat::HeartbeatService;
use pocketclaw_providers::anthropic::AnthropicProvider;
use pocketclaw_providers::google::GoogleProvider;
use pocketclaw_providers::openai::OpenAIProvider;
use pocketclaw_providers::LLMProvider;
use pocketclaw_server::gateway::Gateway;
use pocketclaw_core::channel::ChannelAdapter;
use pocketclaw_telegram::TelegramBot;
use pocketclaw_discord::DiscordBot;
use pocketclaw_tools::exec_tool::ExecTool;
use pocketclaw_tools::fs_tools::{ListDirTool, ReadFileTool, WriteFileTool};
use pocketclaw_tools::registry::ToolRegistry;
use pocketclaw_tools::sandbox::SandboxConfig;
use pocketclaw_tools::web_fetch::WebFetchTool;
use pocketclaw_tools::web_search::WebSearchTool;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

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
    Gateway,
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
}

mod onboard;

fn get_config_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".pocketclaw")
}

fn get_cron_store_path() -> PathBuf {
    get_config_dir().join("cron/jobs.json")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

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
    let sessions = SessionManager::new(workspace.clone(), None);

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

    match &cli.command {
        Some(Commands::Gateway) => {
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
            let cron_store = get_cron_store_path();
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

fn create_provider(config: &AppConfig) -> anyhow::Result<Arc<dyn LLMProvider>> {
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
            check("Telegram Bot", config.telegram.is_some());
            check("Discord Bot", config.discord.is_some());
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
            match std::fs::read_to_string(&skill_path) {
                Ok(content) => {
                    println!("\n📦 Skill: {}", name);
                    println!("----------------------");
                    println!("{}", content);
                }
                Err(_) => println!("✗ Skill '{}' not found", name),
            }
        }
    }
}
