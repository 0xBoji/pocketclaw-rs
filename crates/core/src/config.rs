use serde::{Deserialize, Serialize};
use config::{Config, ConfigError, File};
use std::path::PathBuf;
use crate::attachment::AttachmentPolicy;
use crate::channel::{
    CHANNEL_DISCORD, CHANNEL_GOOGLE_CHAT, CHANNEL_IMESSAGE, CHANNEL_MATRIX, CHANNEL_SIGNAL, CHANNEL_SLACK,
    CHANNEL_TEAMS, CHANNEL_TELEGRAM, CHANNEL_WEBCHAT, CHANNEL_WHATSAPP, CHANNEL_ZALO,
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub workspace: PathBuf,
    pub agents: AgentsConfig,
    pub providers: ProvidersConfig,
    pub whatsapp: Option<WhatsAppConfig>,
    pub telegram: Option<TelegramConfig>,
    pub slack: Option<SlackConfig>,
    pub discord: Option<DiscordConfig>,
    pub signal: Option<SignalConfig>,
    pub imessage: Option<IMessageConfig>,
    pub teams: Option<TeamsConfig>,
    pub matrix: Option<MatrixConfig>,
    pub zalo: Option<ZaloConfig>,
    pub google_chat: Option<GoogleChatConfig>,
    pub webchat: Option<WebChatConfig>,
    pub web: Option<WebConfig>,
    pub google_sheets: Option<GoogleSheetsConfig>,
    pub runtime: Option<RuntimeConfig>,
    #[serde(default)]
    pub attachment_policy: AttachmentPolicy,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuntimeConfig {
    pub ws_heartbeat_secs: Option<u64>,
    pub health_window_minutes: Option<u16>,
    pub dedupe_max_entries: Option<usize>,
    pub adapter_max_inflight: Option<usize>,
    pub adapter_retry_jitter_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DiscordConfig {
    pub token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WhatsAppConfig {
    pub token: String,
    pub api_base: Option<String>,
    pub phone_number_id: Option<String>,
    pub default_to: Option<String>,
    pub verify_token: Option<String>,
    pub app_secret: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SlackConfig {
    pub bot_token: String,
    pub app_token: Option<String>,
    pub default_channel: Option<String>,
    pub signing_secret: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SignalConfig {
    pub endpoint: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IMessageConfig {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TeamsConfig {
    pub bot_token: String,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MatrixConfig {
    pub homeserver: String,
    pub access_token: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ZaloConfig {
    pub token: String,
    pub webhook_url: Option<String>,
    pub default_to: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoogleChatConfig {
    pub webhook_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebChatConfig {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WebConfig {
    pub brave_key: Option<String>,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GoogleSheetsConfig {
    pub spreadsheet_id: String,
    pub service_account_json: String,
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
            PathBuf::from(home).join(".phoneclaw/config.json")
        };

        let s = Config::builder()
            .add_source(File::from(config_path).required(true))
            // Add environment variables (PHONECLAW_...)
            .build()?;

        s.try_deserialize()
    }

    /// Channels configured in the active profile.
    pub fn configured_channels(&self) -> Vec<&'static str> {
        let mut channels = Vec::new();
        if self.whatsapp.is_some() {
            channels.push(CHANNEL_WHATSAPP);
        }
        if self.telegram.is_some() {
            channels.push(CHANNEL_TELEGRAM);
        }
        if self.slack.is_some() {
            channels.push(CHANNEL_SLACK);
        }
        if self.discord.is_some() {
            channels.push(CHANNEL_DISCORD);
        }
        if self.signal.is_some() {
            channels.push(CHANNEL_SIGNAL);
        }
        if self.imessage.as_ref().is_some_and(|cfg| cfg.enabled) {
            channels.push(CHANNEL_IMESSAGE);
        }
        if self.teams.is_some() {
            channels.push(CHANNEL_TEAMS);
        }
        if self.matrix.is_some() {
            channels.push(CHANNEL_MATRIX);
        }
        if self.zalo.is_some() {
            channels.push(CHANNEL_ZALO);
        }
        if self.google_chat.is_some() {
            channels.push(CHANNEL_GOOGLE_CHAT);
        }
        if self.webchat.as_ref().is_some_and(|cfg| cfg.enabled) {
            channels.push(CHANNEL_WEBCHAT);
        }
        channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_json() -> String {
        r#"{
          "workspace": "/tmp/workspace",
          "agents": {
            "default": {
              "model": "openai",
              "system_prompt": "test",
              "max_tokens": 1024,
              "temperature": 0.2
            }
          },
          "providers": {
            "openai": {
              "api_key": "k",
              "api_base": "https://api.openai.com/v1",
              "model": "gpt-4o-mini"
            }
          }
        }"#
        .to_string()
    }

    #[test]
    fn deserializes_multichannel_config() {
        let json = format!(
            r#"{{
              "workspace": "/tmp/workspace",
              "agents": {{
                "default": {{
                  "model": "openai",
                  "system_prompt": "test",
                  "max_tokens": 1024,
                  "temperature": 0.2
                }}
              }},
              "providers": {{
                "openai": {{
                  "api_key": "k",
                  "api_base": "https://api.openai.com/v1",
                  "model": "gpt-4o-mini"
                }}
              }},
              "whatsapp": {{"token": "wa-token"}},
              "telegram": {{"token": "tg-token"}},
              "slack": {{"bot_token": "xoxb", "app_token": "xapp"}},
              "discord": {{"token": "dc-token"}},
              "signal": {{"endpoint": "http://localhost:9000"}},
              "imessage": {{"enabled": true}},
              "teams": {{"bot_token": "teams-token"}},
              "matrix": {{"homeserver": "https://matrix.example", "access_token": "mx-token"}},
              "zalo": {{"token": "zalo-token"}},
              "google_chat": {{"webhook_url": "https://chat.googleapis.com/v1/spaces/AAA/messages?key=K&token=T"}},
              "webchat": {{"enabled": true}}
            }}"#
        );

        let cfg: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
        let configured = cfg.configured_channels();

        assert!(configured.contains(&CHANNEL_WHATSAPP));
        assert!(configured.contains(&CHANNEL_TELEGRAM));
        assert!(configured.contains(&CHANNEL_SLACK));
        assert!(configured.contains(&CHANNEL_DISCORD));
        assert!(configured.contains(&CHANNEL_SIGNAL));
        assert!(configured.contains(&CHANNEL_IMESSAGE));
        assert!(configured.contains(&CHANNEL_TEAMS));
        assert!(configured.contains(&CHANNEL_MATRIX));
        assert!(configured.contains(&CHANNEL_ZALO));
        assert!(configured.contains(&CHANNEL_GOOGLE_CHAT));
        assert!(configured.contains(&CHANNEL_WEBCHAT));
    }

    #[test]
    fn configured_channels_respects_enabled_flags() {
        let json = format!(
            r#"{{
              "workspace": "/tmp/workspace",
              "agents": {{
                "default": {{
                  "model": "openai",
                  "system_prompt": "test",
                  "max_tokens": 1024,
                  "temperature": 0.2
                }}
              }},
              "providers": {{
                "openai": {{
                  "api_key": "k",
                  "api_base": "https://api.openai.com/v1",
                  "model": "gpt-4o-mini"
                }}
              }},
              "imessage": {{"enabled": false}},
              "webchat": {{"enabled": false}}
            }}"#
        );

        let cfg: AppConfig = serde_json::from_str(&json).expect("config should deserialize");
        let configured = cfg.configured_channels();

        assert!(!configured.contains(&CHANNEL_IMESSAGE));
        assert!(!configured.contains(&CHANNEL_WEBCHAT));
    }

    #[test]
    fn minimal_config_still_deserializes() {
        let cfg: AppConfig = serde_json::from_str(&base_json()).expect("minimal config should deserialize");
        assert!(cfg.telegram.is_none());
        assert!(cfg.discord.is_none());
        assert!(cfg.slack.is_none());
        assert!(cfg.configured_channels().is_empty());
    }
}
