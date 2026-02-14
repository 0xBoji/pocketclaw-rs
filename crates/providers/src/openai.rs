use crate::{GenerationOptions, GenerationResponse, LLMProvider, ProviderError, Usage};
use async_trait::async_trait;
use phoneclaw_core::types::{Message, Role};
use reqwest::Client;
use serde_json::{json, Value};

pub struct OpenAIProvider {
    api_key: String,
    api_base: String,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.api_base);

        let messages_json: Vec<Value> = messages
            .iter()
            .map(|m| match m.role {
                Role::Tool => {
                    let tool_call_id = m
                        .metadata
                        .get("tool_call_id")
                        .cloned()
                        .unwrap_or_default();
                    json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": m.content
                    })
                }
                Role::Assistant => {
                    if let Some(tool_calls_json) = m.metadata.get("tool_calls_json") {
                        if let Ok(tool_calls) = serde_json::from_str::<Value>(tool_calls_json) {
                            json!({
                                "role": "assistant",
                                "content": if m.content.is_empty() { Value::Null } else { json!(m.content) },
                                "tool_calls": tool_calls
                            })
                        } else {
                            json!({
                                "role": "assistant",
                                "content": m.content
                            })
                        }
                    } else {
                        json!({
                            "role": "assistant",
                            "content": m.content
                        })
                    }
                }
                _ => {
                    json!({
                        "role": format!("{:?}", m.role).to_lowercase(),
                        "content": m.content
                    })
                }
            })
            .collect();

        let mut body = json!({
            "model": options.model,
            "messages": messages_json,
        });

        if !tools.is_empty() {
             let tools_json: Vec<Value> = tools.iter().map(|t| {
                 json!({
                     "type": "function",
                     "function": t
                 })
             }).collect();
             body["tools"] = json!(tools_json);
             body["tool_choice"] = json!("auto");
        }

        if let Some(max_tokens) = options.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(temperature) = options.temperature {
            body["temperature"] = json!(temperature);
        }

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !res.status().is_success() {
            let error_text = res
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::ApiError(error_text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| ProviderError::ApiError(format!("Failed to parse response: {}", e)))?;

        let choice = &json["choices"][0]["message"];
        let content = choice["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let mut tool_calls = Vec::new();
        if let Some(tcs) = choice.get("tool_calls") {
            if let Some(tcs_array) = tcs.as_array() {
                for tc in tcs_array {
                     let function = &tc["function"];
                     tool_calls.push(crate::ToolCall {
                         id: tc["id"].as_str().unwrap_or_default().to_string(),
                         name: function["name"].as_str().unwrap_or_default().to_string(),
                         arguments: function["arguments"].as_str().unwrap_or_default().to_string(),
                     });
                }
            }
        }

        let usage = if let Some(usage_json) = json.get("usage") {
            Some(Usage {
                input_tokens: usage_json["prompt_tokens"].as_u64().unwrap_or(0) as usize,
                output_tokens: usage_json["completion_tokens"].as_u64().unwrap_or(0) as usize,
            })
        } else {
            None
        };

        Ok(GenerationResponse { content, tool_calls, usage })
    }
}
