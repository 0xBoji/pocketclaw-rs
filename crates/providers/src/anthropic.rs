use super::{GenerationOptions, GenerationResponse, LLMProvider, ProviderError, Usage, ToolCall};
use async_trait::async_trait;
use pocketclaw_core::types::{Message, Role};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct AnthropicProvider {
    api_key: String,
    api_base: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            api_base: "https://api.anthropic.com/v1".to_string(),
            client: Client::new(),
        }
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
        options: &GenerationOptions,
    ) -> Result<GenerationResponse, ProviderError> {
        let url = format!("{}/messages", self.api_base);

        let mut anthropic_messages = Vec::new();
        let mut system_prompt = String::new();

        for m in messages {
            match m.role {
                Role::System => {
                    if !system_prompt.is_empty() {
                        system_prompt.push_str("\n\n");
                    }
                    system_prompt.push_str(&m.content);
                }
                Role::User => {
                    anthropic_messages.push(json!({
                        "role": "user",
                        "content": m.content
                    }));
                }
                Role::Assistant => {
                    anthropic_messages.push(json!({
                        "role": "assistant",
                        "content": m.content
                    }));
                }
                Role::Tool => {
                     // Anthropic tool results format:
                     // {
                     //   "role": "user",
                     //   "content": [
                     //     {
                     //       "type": "tool_result",
                     //       "tool_use_id": "tool_u_...",
                     //       "content": "..."
                     //     }
                     //   ]
                     // }
                     // For simplicity in this port, we interpret Tool role as user message with tool result content.
                     // But Anthropic is strict. 
                     // We need to look up the tool_call_id from metadata.
                     let tool_call_id = m.metadata.get("tool_call_id").cloned().unwrap_or_default();
                     
                     anthropic_messages.push(json!({
                         "role": "user",
                         "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": m.content
                         }]
                     }));
                }
            }
        }
        
        // Anthropic requires alternating user/assistant messages if not using the new specialized format, 
        // but the Messages API is flexible enough if we stick to user/assistant.
        // However, consecutive user messages (like multiple tool results) might need to be merged.
        // For now, let's assume the flow is correct.

        let mut body = json!({
            "model": options.model,
            "messages": anthropic_messages,
            "max_tokens": options.max_tokens.unwrap_or(1024),
        });

        if !system_prompt.is_empty() {
            body["system"] = json!(system_prompt);
        }

        if !tools.is_empty() {
            // Convert JSON Schema tools to Anthropic format
            // Anthropic tools: { name, description, input_schema }
            // Our generic tool defs are already JSON Schema in `parameters` field.
            // But we need to check if our Tool trait returns the full schema or just properties.
            // Our `Tool` trait returns `Value` for parameters.
            // Typically generic tools return `{ "type": "object", "properties": {...} }`.
            // Anthropic expects `input_schema` to be exactly that.
            
            let anthropic_tools: Vec<Value> = tools.iter().map(|t| {
                // `t` here is the tool function definition from OpenAI format: 
                // { "name": "...", "description": "...", "parameters": { ... } }
                // So we can map it directly.
                json!({
                    "name": t["name"],
                    "description": t["description"],
                    "input_schema": t["parameters"]
                })
            }).collect();
            
            body["tools"] = json!(anthropic_tools);
        }

        let res = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !res.status().is_success() {
             let error_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
             return Err(ProviderError::ApiError(error_text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| ProviderError::ApiError(format!("Failed to parse response: {}", e)))?;

        // Parse content
        let mut content = String::new();
        let mut tool_calls = Vec::new();

        if let Some(content_array) = json["content"].as_array() {
            for item in content_array {
                match item["type"].as_str() {
                    Some("text") => {
                        content.push_str(item["text"].as_str().unwrap_or_default());
                    }
                    Some("tool_use") => {
                        tool_calls.push(ToolCall {
                            id: item["id"].as_str().unwrap_or_default().to_string(),
                            name: item["name"].as_str().unwrap_or_default().to_string(),
                            arguments: item["input"].to_string(), // Anthropic returns object, we need string for our trait
                        });
                    }
                    _ => {}
                }
            }
        }
        
        let usage = if let Some(usage_json) = json.get("usage") {
             Some(Usage {
                 input_tokens: usage_json["input_tokens"].as_u64().unwrap_or(0) as usize,
                 output_tokens: usage_json["output_tokens"].as_u64().unwrap_or(0) as usize,
             })
        } else {
             None
        };

        Ok(GenerationResponse {
            content,
            tool_calls,
            usage,
        })
    }
}
