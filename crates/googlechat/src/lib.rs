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
pub struct GoogleChatAdapter {
    bus: Arc<MessageBus>,
    webhook_url: String,
    max_inflight: usize,
    retry_jitter_ms: u64,
    client: Client,
}

impl GoogleChatAdapter {
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

    async fn send_outbound_with_retry(&self, thread_key: Option<&str>, message: &str) -> anyhow::Result<()> {
        let url = build_google_chat_url(&self.webhook_url, thread_key);
        let payload = json!({ "text": message });

        let mut delay = Duration::from_secs(1);
        for attempt in 1..=3 {
            let resp = self.client.post(&url).json(&payload).send().await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    info!(attempt, "Google Chat outbound sent");
                    return Ok(());
                }
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    warn!(attempt, %status, body = %body, "Google Chat outbound failed");
                }
                Err(e) => {
                    warn!(attempt, error = %e, "Google Chat outbound network error");
                }
            }

            sleep(delay + self.jitter_delay(attempt)).await;
            delay *= 2;
        }

        anyhow::bail!("failed to deliver Google Chat message after retries")
    }
}

fn build_google_chat_url(base: &str, thread_key: Option<&str>) -> String {
    let Some(thread_key) = thread_key.filter(|k| !k.is_empty()) else {
        return base.to_string();
    };

    if base.contains('?') {
        format!("{}&threadKey={}", base, thread_key)
    } else {
        format!("{}?threadKey={}", base, thread_key)
    }
}

#[async_trait]
impl ChannelAdapter for GoogleChatAdapter {
    fn channel_name(&self) -> &str {
        "google_chat"
    }

    async fn start(&self) -> anyhow::Result<()> {
        info!("Starting Google Chat adapter");

        let bus = self.bus.clone();
        let webhook_url = self.webhook_url.clone();
        let max_inflight = self.max_inflight;
        let retry_jitter_ms = self.retry_jitter_ms;
        let client = self.client.clone();
        let semaphore = Arc::new(Semaphore::new(self.max_inflight));

        tokio::spawn(async move {
            let adapter = GoogleChatAdapter {
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
                        if !msg.session_key.starts_with("google_chat:") {
                            continue;
                        }

                        let thread_key = msg.session_key.strip_prefix("google_chat:").filter(|s| !s.is_empty());

                        let permit = match semaphore.clone().acquire_owned().await {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        let adapter_cloned = adapter.clone();
                        let content = msg.content.clone();
                        let thread_key = thread_key.map(|s| s.to_string());

                        tokio::spawn(async move {
                            let _permit = permit;
                            if let Err(e) = adapter_cloned
                                .send_outbound_with_retry(thread_key.as_deref(), &content)
                                .await
                            {
                                error!(error = %e, "Google Chat outbound failed permanently");
                            }
                        });
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!(error = %e, "Google Chat bus subscription error");
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(())
    }
}
