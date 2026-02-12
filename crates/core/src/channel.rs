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
