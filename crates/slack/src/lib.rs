use async_trait::async_trait;
use phoneclaw_core::bus::{Event, MessageBus};
use phoneclaw_core::channel::ChannelAdapter;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};
use tokio::time::{sleep, Duration};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct SlackAdapter {
    bus: Arc<MessageBus>,
    bot_token: String,
    default_channel: Option<String>,
    max_inflight: usize,
    retry_jitter_ms: u64,
    client: Client,
}

impl SlackAdapter {
    pub fn new(
        bus: Arc<MessageBus>,
        bot_token: String,
        default_channel: Option<String>,
        max_inflight: usize,
        retry_jitter_ms: u64,
    ) -> Self {
        Self {
            bus,
            bot_token,
            default_channel,
            max_inflight: max_inflight.max(1),
            retry_jitter_ms,
            client: Client::new(),
        }
    }

    fn jitter_delay(&self, attempt: u32) -> StdDuration {
        if self.retry_jitter_ms == 0 {
            return StdDuration::from_millis(0);
        }
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or_default();
        let jitter = (seed ^ ((attempt as u64) << 7)) % (self.retry_jitter_ms + 1);
        StdDuration::from_millis(jitter)
    }

    async fn send_outbound_with_retry(
        &self,
        channel: &str,
        message: &str,
        thread_ts: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut payload = json!({
            "channel": channel,
            "text": message,
        });
        if let Some(ts) = thread_ts {
            payload["thread_ts"] = json!(ts);
        }

        let mut delay = Duration::from_secs(1);
        for attempt in 1..=3 {
            let resp = self
                .client
                .post("https://slack.com/api/chat.postMessage")
                .bearer_auth(&self.bot_token)
                .json(&payload)
                .send()
                .await;

            match resp {
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    let ok = serde_json::from_str::<serde_json::Value>(&body)
                        .ok()
                        .and_then(|v| v.get("ok").and_then(|x| x.as_bool()))
                        .unwrap_or(false);

                    if status.is_success() && ok {
                        info!(channel = %channel, attempt, "Slack outbound sent");
                        return Ok(());
                    }

                    warn!(attempt, %status, body = %body, "Slack outbound failed");
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Slack outbound network error");
                }
            }

            sleep(delay + self.jitter_delay(attempt)).await;
            delay *= 2;
        }

        anyhow::bail!("failed to deliver Slack message after retries");
    }
}

#[async_trait]
impl ChannelAdapter for SlackAdapter {
    fn channel_name(&self) -> &str {
        "slack"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting Slack adapter");

        let bus = self.bus.clone();
        let bot_token = self.bot_token.clone();
        let default_channel = self.default_channel.clone();
        let max_inflight = self.max_inflight;
        let retry_jitter_ms = self.retry_jitter_ms;
        let client = self.client.clone();
        let semaphore = Arc::new(Semaphore::new(self.max_inflight));

        tokio::spawn(async move {
            let adapter = SlackAdapter {
                bus: bus.clone(),
                bot_token,
                default_channel,
                max_inflight,
                retry_jitter_ms,
                client,
            };

            let mut rx = bus.subscribe();
            loop {
                match rx.recv().await {
                    Ok(Event::OutboundMessage(msg)) => {
                        if !msg.session_key.starts_with("slack:") {
                            continue;
                        }

                        let raw = msg.session_key.strip_prefix("slack:").unwrap_or_default();
                        let mut parts = raw.split(':');
                        let session_channel = parts.next().filter(|s| !s.is_empty()).map(|s| s.to_string());
                        let thread_ts = parts.next().filter(|s| !s.is_empty());
                        let channel = session_channel.or_else(|| adapter.default_channel.clone());

                        let Some(channel) = channel else {
                            warn!("Slack outbound dropped: no target channel configured");
                            continue;
                        };

                        let permit = match semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let adapter_cloned = adapter.clone();
                        let content = msg.content.clone();
                        let channel = channel.clone();
                        let thread_ts_owned = thread_ts.map(|s| s.to_string());

                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = adapter_cloned
                                .send_outbound_with_retry(
                                    &channel,
                                    &content,
                                    thread_ts_owned.as_deref(),
                                )
                                .await
                            {
                                error!(error = %e, "Slack outbound failed permanently");
                            }
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!(error = %e, "Slack bus subscription error");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}
