use crate::sandbox::SandboxConfig;
use crate::{Tool, ToolError};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

pub struct WebFetchTool {
    client: Client,
    sandbox: SandboxConfig,
}

impl WebFetchTool {
    pub fn new(sandbox: SandboxConfig) -> Self {
        Self {
            client: Client::builder()
                .user_agent("PocketClaw/1.0")
                .build()
                .unwrap_or_default(),
            sandbox,
        }
    }
}

#[derive(Deserialize)]
struct FetchArgs {
    url: String,
}

/// Extract domain from a URL string (simple, no external dep)
fn extract_domain(url: &str) -> Option<String> {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let domain = without_scheme.split('/').next()?;
    let domain = domain.split(':').next()?; // strip port
    Some(domain.to_lowercase())
}

/// Simple HTML to text extractor (no OpenSSL dependency)
fn html_to_text(html: &str) -> (String, String) {
    let title = regex::Regex::new(r"(?is)<title>(.*?)</title>")
        .ok()
        .and_then(|re| re.captures(html))
        .map(|c| c[1].trim().to_string())
        .unwrap_or_default();

    let re_script = regex::Regex::new(r"(?is)<(script|style|noscript)[^>]*>.*?</\1>").unwrap();
    let cleaned = re_script.replace_all(html, "");

    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let text = re_tags.replace_all(&cleaned, " ");

    let text = text
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

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

        // Network allowlist check
        if !self.sandbox.network_allowlist.is_empty() {
            let domain = extract_domain(&args.url).unwrap_or_default();
            if !self.sandbox.network_allowlist.iter().any(|d| domain.ends_with(d.as_str())) {
                return Err(ToolError::ExecutionError(format!(
                    "Domain '{}' is not in the network allowlist",
                    domain
                )));
            }
        }

        info!(url = %args.url, "Fetching URL");

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

        // 2. Extract text content
        let (title, text) = html_to_text(&html);

        // Truncate if too long (respect sandbox output limit)
        let max_len = std::cmp::min(8000, self.sandbox.max_output_bytes);
        let content = if text.len() > max_len {
            format!("{}...\n\n[Truncated at {} chars]", &text[..max_len], max_len)
        } else {
            text
        };

        Ok(format!("Title: {}\n\nContent:\n{}", title, content))
    }
}
