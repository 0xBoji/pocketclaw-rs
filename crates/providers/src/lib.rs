pub mod google;
pub mod anthropic;
pub mod openai;
pub mod factory;
use async_trait::async_trait;
use phoneclaw_core::types::Message;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Debug, Clone)]
pub struct GenerationOptions {
    pub model: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct GenerationResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}


#[derive(Debug, Clone)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError>;
}
