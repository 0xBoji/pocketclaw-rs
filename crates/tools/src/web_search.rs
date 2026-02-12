use crate::sandbox::SandboxConfig;
use crate::{Tool, ToolError};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

pub struct WebSearchTool {
    api_key: String,
    client: Client,
    #[allow(dead_code)]
    sandbox: SandboxConfig,
}

impl WebSearchTool {
    pub fn new(api_key: String, sandbox: SandboxConfig) -> Self {
        Self {
            api_key,
            client: Client::new(),
            sandbox,
        }
    }
}

#[derive(Deserialize)]
struct BraveSearchResponse {
    web: Option<BraveSearchWeb>,
}

#[derive(Deserialize)]
struct BraveSearchWeb {
    results: Vec<BraveSearchResult>,
}

#[derive(Deserialize)]
struct BraveSearchResult {
    title: String,
    url: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information using Brave Search."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let args: SearchArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // Audit log — always log search queries
        info!(query = %args.query, "Web search query");

        let url = "https://api.search.brave.com/res/v1/web/search";
        
        let res = self.client
            .get(url)
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .query(&[("q", &args.query)])
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        if !res.status().is_success() {
            return Err(ToolError::ExecutionError(format!("Search API failed: {}", res.status())));
        }

        let data: BraveSearchResponse = res.json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to parse search response: {}", e)))?;

        let mut output = String::new();
        if let Some(web) = data.web {
            for result in web.results.iter().take(5) {
                output.push_str(&format!("Title: {}\nURL: {}\nDescription: {}\n\n", 
                    result.title, 
                    result.url, 
                    result.description.as_deref().unwrap_or("No description")
                ));
            }
        } else {
            output.push_str("No results found.");
        }

        Ok(output)
    }
}
