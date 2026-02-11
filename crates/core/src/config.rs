use serde::{Deserialize, Serialize};
use config::{Config, ConfigError, File};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub workspace: PathBuf,
    pub agents: AgentsConfig,
    pub providers: ProvidersConfig,
    pub telegram: Option<TelegramConfig>,
    pub discord: Option<DiscordConfig>,
    pub web: Option<WebConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DiscordConfig {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebConfig {
    pub brave_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TelegramConfig {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentsConfig {
    pub default: AgentDefaultConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentDefaultConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: usize,
    pub temperature: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ProvidersConfig {
    pub openai: Option<ProviderConfig>,
    pub openrouter: Option<ProviderConfig>,
    pub anthropic: Option<AnthropicConfig>,
    pub google: Option<GoogleConfig>,
    pub groq: Option<GroqConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GroqConfig {
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoogleConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProviderConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
}

impl AppConfig {
    pub fn load(custom_path: Option<PathBuf>) -> Result<Self, ConfigError> {
        let config_path = if let Some(path) = custom_path {
            path
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".pocketclaw/config.json")
        };

        let s = Config::builder()
            .add_source(File::from(config_path).required(true))
            // Add environment variables (POCKETCLAW_...)
            .build()?;

        s.try_deserialize()
    }
}
