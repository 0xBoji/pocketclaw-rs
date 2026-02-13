use inquire::{Confirm, Password, Select, Text};
use pocketclaw_core::config::{
    AgentDefaultConfig, AgentsConfig, AnthropicConfig, AppConfig, DiscordConfig, GoogleConfig,
    GroqConfig, ProviderConfig, ProvidersConfig, TelegramConfig, WebConfig,
};
use pocketclaw_core::attachment::AttachmentPolicy;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_onboarding() -> anyhow::Result<()> {
    println!("🦞 Welcome to PocketClaw Setup Wizard!");
    println!("This wizard will help you generate a configuration file.\n");

    let workspace_str = Text::new("Where should the workspace be located?")
        .with_default("workspace")
        .prompt()?;
    let workspace = PathBuf::from(&workspace_str);

    // --- Providers ---
    let mut providers_config = ProvidersConfig::default();

    // OpenAI
    if Confirm::new("Configure OpenAI/Compatible Provider?")
        .with_default(true)
        .prompt()?
    {
        let api_key = Password::new("Enter OpenAI API Key:")
            .without_confirmation()
            .prompt()?;
        let api_base = Text::new("API Base URL (optional):")
            .with_default("https://api.openai.com/v1")
            .prompt()?;
        let model = Text::new("Model Name:")
            .with_default("gpt-4o")
            .prompt()?;

        providers_config.openai = Some(ProviderConfig {
            api_key,
            api_base: Some(api_base),
            model,
        });
    }

    // Anthropic
    if Confirm::new("Configure Anthropic (Claude)?")
        .with_default(false)
        .prompt()?
    {
        let api_key = Password::new("Enter Anthropic API Key:")
            .without_confirmation()
            .prompt()?;
        let model = Text::new("Model Name:")
            .with_default("claude-3-opus-20240229")
            .prompt()?;

        providers_config.anthropic = Some(AnthropicConfig { api_key, model });
    }

    // Google Gemini
    if Confirm::new("Configure Google Gemini?")
        .with_default(false)
        .prompt()?
    {
        let api_key = Password::new("Enter Google AI Studio API Key:")
            .without_confirmation()
            .prompt()?;
        let model = Text::new("Model Name:")
            .with_default("gemini-1.5-flash")
            .prompt()?;

        providers_config.google = Some(GoogleConfig { api_key, model });
    }

    // Groq (Voice Transcription)
    if Confirm::new("Configure Groq (Voice Transcription)?")
        .with_default(false)
        .prompt()?
    {
        let api_key = Password::new("Enter Groq API Key:")
            .without_confirmation()
            .prompt()?;
        providers_config.groq = Some(GroqConfig { api_key });
    }

    // --- Agents ---
    let _agent_name = Text::new("Name your agent:")
        .with_default("PocketClaw")
        .prompt()?;
    let agent_prompt = Text::new("System Prompt:")
        .with_default("You are a helpful assistant.")
        .prompt()?;

    let model_choice =
        if providers_config.anthropic.is_some() && providers_config.openai.is_some() {
            Select::new(
                "Which provider should be default?",
                vec!["openai", "anthropic"],
            )
            .prompt()?
        } else if providers_config.anthropic.is_some() {
            "anthropic"
        } else {
            "openai"
        };

    let agents_config = AgentsConfig {
        default: AgentDefaultConfig {
            model: model_choice.to_string(),
            system_prompt: agent_prompt,
            max_tokens: 4096,
            temperature: 0.7,
        },
    };

    // --- Integrations ---
    let mut telegram_config = None;
    if Confirm::new("Configure Telegram Bot?")
        .with_default(false)
        .prompt()?
    {
        let token = Password::new("Telegram Bot Token:")
            .without_confirmation()
            .prompt()?;
        telegram_config = Some(TelegramConfig { token });
    }

    let mut discord_config = None;
    if Confirm::new("Configure Discord Bot?")
        .with_default(false)
        .prompt()?
    {
        let token = Password::new("Discord Bot Token:")
            .without_confirmation()
            .prompt()?;
        discord_config = Some(DiscordConfig { token });
    }

    let mut web_config = None;
    if Confirm::new("Configure Web Search (Brave)?")
        .with_default(false)
        .prompt()?
    {
        let key = Password::new("Brave Search API Key:")
            .without_confirmation()
            .prompt()?;
        web_config = Some(WebConfig {
            brave_key: Some(key),
            auth_token: None,
        });
    }

    // --- Final Config ---
    let config = AppConfig {
        workspace: workspace.clone(),
        agents: agents_config,
        providers: providers_config,
        whatsapp: None,
        telegram: telegram_config,
        slack: None,
        discord: discord_config,
        signal: None,
        imessage: None,
        teams: None,
        matrix: None,
        zalo: None,
        google_chat: None,
        webchat: None,
        web: web_config,
        google_sheets: None,
        runtime: None,
        attachment_policy: AttachmentPolicy::default(),
    };

    let config_json = serde_json::to_string_pretty(&config)?;

    // Save config
    let config_path = dirs::home_dir()
        .unwrap()
        .join(".pocketclaw/config.json");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if config_path.exists()
        && !Confirm::new("Config file already exists. Overwrite?")
            .with_default(false)
            .prompt()?
    {
        println!("Aborted.");
        return Ok(());
    }

    fs::write(&config_path, config_json)?;

    // Create workspace directories & templates
    create_workspace(&workspace)?;

    println!("\n🦞 PocketClaw is ready!");
    println!("\nNext steps:");
    println!("  1. Review your config: {:?}", config_path);
    println!("  2. Chat: pocketclaw agent -m \"Hello!\"");

    Ok(())
}

fn create_workspace(workspace: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(workspace)?;
    fs::create_dir_all(workspace.join("memory"))?;
    fs::create_dir_all(workspace.join("skills"))?;

    let templates: Vec<(&str, &str)> = vec![
        (
            "AGENTS.md",
            r#"# Agent Instructions

You are a helpful AI assistant. Be concise, accurate, and friendly.

## Guidelines

- Always explain what you're doing before taking actions
- Ask for clarification when request is ambiguous
- Use tools to help accomplish tasks
- Remember important information in your memory files
- Be proactive and helpful
- Learn from user feedback
"#,
        ),
        (
            "SOUL.md",
            r#"# Soul

I am pocketclaw, a lightweight AI assistant powered by AI.

## Personality

- Helpful and friendly
- Concise and to the point
- Curious and eager to learn
- Honest and transparent

## Values

- Accuracy over speed
- User privacy and safety
- Transparency in actions
- Continuous improvement
"#,
        ),
        (
            "USER.md",
            r#"# User

Information about user goes here.

## Preferences

- Communication style: (casual/formal)
- Timezone: (your timezone)
- Language: (your preferred language)

## Personal Information

- Name: (optional)
- Location: (optional)
- Occupation: (optional)
"#,
        ),
        (
            "TOOLS.md",
            r#"# Available Tools

## File Operations
- Read file contents
- Write/Create files
- List directories

## Web Tools
- Web Search (Brave API)
- Web Fetch (extract readable content)

## Command Execution
- Execute shell commands in workspace
- Full shell access with timeout protection
"#,
        ),
        (
            "IDENTITY.md",
            r#"# Identity

## Name
PicoClaw 🦞

## Description
Ultra-lightweight personal AI assistant written in Rust.

## Purpose
- Provide intelligent AI assistance with minimal resource usage
- Support multiple LLM providers (OpenAI, Anthropic, Google, etc.)
- Enable easy customization through skills system
- Run on minimal hardware

## Philosophy
- Simplicity over complexity
- Performance over features
- User control and privacy
- Transparent operation
"#,
        ),
    ];

    for (filename, content) in &templates {
        let file_path = workspace.join(filename);
        if !file_path.exists() {
            fs::write(&file_path, content)?;
            println!("  Created {}", filename);
        }
    }

    // Memory file
    let memory_file = workspace.join("memory/MEMORY.md");
    if !memory_file.exists() {
        fs::write(
            &memory_file,
            r#"# Long-term Memory

This file stores important information that should persist across sessions.

## User Information

(Important facts about user)

## Preferences

(User preferences learned over time)

## Important Notes

(Things to remember)
"#,
        )?;
        println!("  Created memory/MEMORY.md");
    }

    Ok(())
}
