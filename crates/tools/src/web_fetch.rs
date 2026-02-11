use crate::{Tool, ToolError};
use async_trait::async_trait;
use readability::extractor;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Cursor;

pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("PocketClaw/1.0")
                .build()
                .unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
struct FetchArgs {
    url: String,
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch and extract the main content from a webpage."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let args: FetchArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // 1. Fetch HTML
        let res = self.client
            .get(&args.url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to fetch URL: {}", e)))?;

        if !res.status().is_success() {
             return Err(ToolError::ExecutionError(format!("HTTP Error: {}", res.status())));
        }

        let html = res.text()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to read text: {}", e)))?;

        // 2. Extract Content using readability
        let mut cursor = Cursor::new(html);
        let product = extractor::extract(&mut cursor, &reqwest::Url::parse(&args.url).unwrap())
            .map_err(|e| ToolError::ExecutionError(format!("Readability error: {}", e)))?;

        Ok(format!("Title: {}\n\nContent:\n{}", product.title, product.text))
    }
}
