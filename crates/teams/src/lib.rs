use async_trait::async_trait;
use phoneclaw_core::bus::{Event, MessageBus};
use phoneclaw_core::channel::ChannelAdapter;
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct TeamsAdapter {
    bus: Arc<MessageBus>,
    webhook_url: String,
    max_inflight: usize,
    retry_jitter_ms: u64,
    client: Client,
}

impl TeamsAdapter {
    pub fn new(
        bus: Arc<MessageBus>,
        webhook_url: String,
        max_inflight: usize,
        retry_jitter_ms: u64,
    ) -> Self {
        Self {
            bus,
            webhook_url,
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

    async fn send_outbound_with_retry(&self, message: &str) -> anyhow::Result<()> {
        let payload = json!({ "text": message });

        let mut delay = Duration::from_secs(1);
        for attempt in 1..=3 {
            let resp = self.client.post(&self.webhook_url).json(&payload).send().await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    info!(attempt, "Teams outbound sent");
                    return Ok(());
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    warn!(attempt, %status, body = %body, "Teams outbound failed");
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Teams outbound network error");
                }
            }

            sleep(delay + self.jitter_delay(attempt)).await;
            delay *= 2;
        }

        anyhow::bail!("failed to deliver Teams message after retries")
    }
}

#[async_trait]
impl ChannelAdapter for TeamsAdapter {
    fn channel_name(&self) -> &str {
        "teams"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting Teams adapter");

        let bus = self.bus.clone();
        let webhook_url = self.webhook_url.clone();
        let max_inflight = self.max_inflight;
        let retry_jitter_ms = self.retry_jitter_ms;
        let client = self.client.clone();
        let semaphore = Arc::new(Semaphore::new(self.max_inflight));

        tokio::spawn(async move {
            let adapter = TeamsAdapter {
                bus: bus.clone(),
                webhook_url,
                max_inflight,
                retry_jitter_ms,
                client,
            };

            let mut rx = bus.subscribe();
            loop {
                match rx.recv().await {
                    Ok(Event::OutboundMessage(msg)) => {
                        if !msg.session_key.starts_with("teams:") {
                            continue;
                        }

                        let permit = match semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let adapter_cloned = adapter.clone();
                        let content = msg.content.clone();

                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = adapter_cloned.send_outbound_with_retry(&content).await {
                                error!(error = %e, "Teams outbound failed permanently");
                            }
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!(error = %e, "Teams bus subscription error");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}
