use super::{GenerationOptions, GenerationResponse, LLMProvider, ProviderError, Usage};
use async_trait::async_trait;
use phoneclaw_core::types::{Message, Role};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone)]
pub struct GoogleProvider {
    client: Client,
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct GoogleContent {
    role: String,
    parts: Vec<GooglePart>,
}

#[derive(Serialize)]
struct GooglePart {
    text: String,
}

#[derive(Deserialize)]
struct GoogleResponse {
    candidates: Option<Vec<GoogleCandidate>>,
    error: Option<GoogleError>,
}

#[derive(Deserialize)]
struct GoogleCandidate {
    content: Option<GoogleContentPart>,
    #[serde(rename = "finishReason")]
    _finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GoogleContentPart {
    parts: Option<Vec<GoogleResponsePart>>,
}

#[derive(Deserialize)]
struct GoogleResponsePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct GoogleError {
    code: i32,
    message: String,
}

impl GoogleProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl LLMProvider for GoogleProvider {
    async fn chat(
        &self,
        messages: &[Message],
        _tools: &[serde_json::Value],
        _options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let contents: Vec<GoogleContent> = messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "model",
                    Role::System => "user",
                    _ => "user",
                };
                
                GoogleContent {
                    role: role.to_string(),
                    parts: vec![GooglePart { text: m.content.clone() }],
                }
            })
            .collect();

        let body = json!({
            "contents": contents,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !resp.status().is_success() {
             let error_text = resp.text().await.unwrap_or_default();
             return Err(ProviderError::ApiError(format!("Google API Error: {}", error_text)));
        }

        let google_resp: GoogleResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ApiError(format!("Serialization Error: {}", e)))?;

        if let Some(error) = google_resp.error {
            return Err(ProviderError::ApiError(format!(
                "Google Error {}: {}",
                error.code, error.message
            )));
        }

        let candidate = google_resp
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .ok_or_else(|| ProviderError::ApiError("No candidates returned".to_string()))?;

        let text = candidate
            .content
            .as_ref()
            .and_then(|c| c.parts.as_ref())
            .and_then(|p| p.first())
            .and_then(|p| p.text.clone())
            .unwrap_or_default();

        Ok(GenerationResponse {
            content: text,
            tool_calls: vec![],
            usage: Some(Usage { input_tokens: 0, output_tokens: 0 }),
        })
    }
}
