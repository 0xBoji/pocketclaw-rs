use crate::{Tool, ToolError};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

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

/// Simple HTML to text extractor (no OpenSSL dependency)
fn html_to_text(html: &str) -> (String, String) {
    // Extract title
    let title = regex::Regex::new(r"(?is)<title>(.*?)</title>")
        .ok()
        .and_then(|re| re.captures(html))
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    // Remove script and style blocks
    let re_script = regex::Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>").unwrap();
    let cleaned = re_script.replace_all(html, "");

    // Remove HTML tags
    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&cleaned, " ");

    // Decode basic HTML entities
    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse whitespace
    let re_ws = regex::Regex::new(r"\s+").unwrap();
    let text = re_ws.replace_all(&text, " ").trim().to_string();

    (title, text)
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

        // 2. Extract text content (lightweight, no OpenSSL)
        let (title, text) = html_to_text(&html);

        // Truncate if too long
        let max_len = 8000;
        let content = if text.len() > max_len {
            format!("{}...\n\n[Truncated at {} chars]", &text[..max_len], max_len)
        } else {
            text
        };

        Ok(format!("Title: {}\n\nContent:\n{}", title, content))
    }
}
