use async_trait::async_trait;

/// Standardized interface for all channel adapters (Telegram, Discord, etc.).
/// Each adapter runs as a Tokio task, consuming inbound messages from its platform
/// and publishing them to the MessageBus, while subscribing to outbound messages.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Unique channel identifier (e.g., "telegram", "discord", "gateway").
    fn channel_name(&self) -> &str;

    /// Start the adapter. This should spawn the necessary inbound/outbound tasks.
    /// Typically blocks or runs until the adapter is stopped.
    async fn start(&self) -> anyhow::Result<()>;

    /// Quick health check — verify the adapter connection is alive.
    async fn health_check(&self) -> bool {
        true // default: assume healthy
    }
}

pub const CHANNEL_WHATSAPP: &str = "whatsapp";
pub const CHANNEL_TELEGRAM: &str = "telegram";
pub const CHANNEL_SLACK: &str = "slack";
pub const CHANNEL_DISCORD: &str = "discord";
pub const CHANNEL_SIGNAL: &str = "signal";
pub const CHANNEL_IMESSAGE: &str = "imessage";
pub const CHANNEL_TEAMS: &str = "teams";
pub const CHANNEL_MATRIX: &str = "matrix";
pub const CHANNEL_ZALO: &str = "zalo";
pub const CHANNEL_GOOGLE_CHAT: &str = "google_chat";
pub const CHANNEL_WEBCHAT: &str = "webchat";

/// Target personal-assistant channel set.
pub fn target_personal_channels() -> [&'static str; 11] {
    [
        CHANNEL_WHATSAPP,
        CHANNEL_TELEGRAM,
        CHANNEL_SLACK,
        CHANNEL_DISCORD,
        CHANNEL_SIGNAL,
        CHANNEL_IMESSAGE,
        CHANNEL_TEAMS,
        CHANNEL_MATRIX,
        CHANNEL_ZALO,
        CHANNEL_GOOGLE_CHAT,
        CHANNEL_WEBCHAT,
    ]
}

/// Native runtime channel adapters currently implemented in Rust.
pub fn native_supported_channels() -> [&'static str; 7] {
    [
        CHANNEL_WHATSAPP,
        CHANNEL_TELEGRAM,
        CHANNEL_DISCORD,
        CHANNEL_SLACK,
        CHANNEL_TEAMS,
        CHANNEL_ZALO,
        CHANNEL_GOOGLE_CHAT,
    ]
}

pub fn is_native_channel_supported(channel: &str) -> bool {
    native_supported_channels().contains(&channel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_set_contains_requested_channels() {
        let channels = target_personal_channels();
        assert!(channels.contains(&CHANNEL_WHATSAPP));
        assert!(channels.contains(&CHANNEL_TELEGRAM));
        assert!(channels.contains(&CHANNEL_SLACK));
        assert!(channels.contains(&CHANNEL_DISCORD));
        assert!(channels.contains(&CHANNEL_SIGNAL));
        assert!(channels.contains(&CHANNEL_IMESSAGE));
        assert!(channels.contains(&CHANNEL_TEAMS));
        assert!(channels.contains(&CHANNEL_MATRIX));
        assert!(channels.contains(&CHANNEL_ZALO));
        assert!(channels.contains(&CHANNEL_GOOGLE_CHAT));
        assert!(channels.contains(&CHANNEL_WEBCHAT));
    }

    #[test]
    fn support_matrix_is_explicit() {
        assert!(is_native_channel_supported(CHANNEL_WHATSAPP));
        assert!(is_native_channel_supported(CHANNEL_TELEGRAM));
        assert!(is_native_channel_supported(CHANNEL_DISCORD));
        assert!(is_native_channel_supported(CHANNEL_SLACK));
        assert!(is_native_channel_supported(CHANNEL_TEAMS));
        assert!(is_native_channel_supported(CHANNEL_ZALO));
        assert!(is_native_channel_supported(CHANNEL_GOOGLE_CHAT));
        assert!(!is_native_channel_supported(CHANNEL_WEBCHAT));
    }
}
