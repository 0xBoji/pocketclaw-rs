use crate::{GenerationOptions, GenerationResponse, LLMProvider, ProviderError};
use async_trait::async_trait;
use phoneclaw_core::types::Message;
use std::sync::Arc;
use std::time::Duration;

/// Lightweight reliability wrapper for unstable mobile networks.
/// Retries transient failures with bounded exponential backoff.
pub struct ReliableProvider {
    inner: Arc<dyn LLMProvider>,
    max_retries: u32,
    base_backoff_ms: u64,
}

pub struct FailoverProvider {
    providers: Vec<(String, Arc<dyn LLMProvider>, Option<String>)>,
}

impl ReliableProvider {
    pub fn new(inner: Arc<dyn LLMProvider>, max_retries: u32, base_backoff_ms: u64) -> Self {
        Self {
            inner,
            max_retries,
            base_backoff_ms: base_backoff_ms.max(100),
        }
    }

    fn is_retryable(err: &ProviderError) -> bool {
        match err {
            ProviderError::NetworkError(_) => true,
            ProviderError::ApiError(message) => {
                let lower = message.to_lowercase();
                lower.contains("429")
                    || lower.contains("rate limit")
                    || lower.contains("too many requests")
                    || lower.contains("timeout")
                    || lower.contains("temporar")
                    || lower.contains("unavailable")
                    || lower.contains("503")
            }
            ProviderError::ConfigError(_) => false,
        }
    }
}

impl FailoverProvider {
    pub fn new(providers: Vec<(String, Arc<dyn LLMProvider>, Option<String>)>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl LLMProvider for FailoverProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError> {
        if self.providers.is_empty() {
            return Err(ProviderError::ConfigError(
                "no providers available for failover".to_string(),
            ));
        }

        let mut failures = Vec::new();
        for (name, provider, model_override) in &self.providers {
            let mut provider_options = options.clone();
            if let Some(model) = model_override {
                provider_options.model = model.clone();
            }
            match provider.chat(messages, tools, &provider_options).await {
                Ok(resp) => {
                    if !failures.is_empty() {
                        tracing::warn!(
                            provider = name,
                            previous_failures = failures.len(),
                            "provider failover recovered"
                        );
                    }
                    return Ok(resp);
                }
                Err(err) => {
                    failures.push(format!("{name}: {err}"));
                    tracing::warn!(provider = name, "provider failed, trying next");
                }
            }
        }

        Err(ProviderError::ApiError(format!(
            "all providers failed; {}",
            failures.join(" | ")
        )))
    }
}

#[async_trait]
impl LLMProvider for ReliableProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError> {
        let mut backoff_ms = self.base_backoff_ms;
        let mut last_err: Option<ProviderError> = None;

        for attempt in 0..=self.max_retries {
            match self.inner.chat(messages, tools, options).await {
                Ok(resp) => return Ok(resp),
                Err(err) => {
                    if !Self::is_retryable(&err) || attempt == self.max_retries {
                        return Err(err);
                    }

                    tracing::warn!(
                        attempt = attempt + 1,
                        max_attempts = self.max_retries + 1,
                        backoff_ms,
                        "provider call failed; retrying"
                    );
                    last_err = Some(err);
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms.saturating_mul(2)).min(2_000);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| ProviderError::ApiError("unknown provider failure".into())))
    }
}
