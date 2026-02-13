use crate::{Tool, ToolError};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};

pub struct ChannelHealthTool {
    client: Client,
    base_url: String,
    auth_token: Option<String>,
}

impl ChannelHealthTool {
    pub fn new(base_url: String, auth_token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url,
            auth_token,
        }
    }
}

#[async_trait]
impl Tool for ChannelHealthTool {
    fn name(&self) -> &str {
        "channel_health"
    }

    fn description(&self) -> &str {
        "Fetch channel runtime health and trend from the local gateway."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> Result<String, ToolError> {
        let mut req = self
            .client
            .get(format!("{}/api/channels/health", self.base_url.trim_end_matches('/')));
        if let Some(token) = &self.auth_token {
            if !token.is_empty() {
                req = req.bearer_auth(token);
            }
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ToolError::ExecutionError(format!(
                "Gateway health request failed: {}",
                resp.status()
            )));
        }
        let data: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        serde_json::to_string_pretty(&data).map_err(|e| ToolError::ExecutionError(e.to_string()))
    }
}

pub struct MetricsSnapshotTool {
    client: Client,
    base_url: String,
    auth_token: Option<String>,
}

impl MetricsSnapshotTool {
    pub fn new(base_url: String, auth_token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url,
            auth_token,
        }
    }
}

#[async_trait]
impl Tool for MetricsSnapshotTool {
    fn name(&self) -> &str {
        "metrics_snapshot"
    }

    fn description(&self) -> &str {
        "Fetch gateway metrics snapshot."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> Result<String, ToolError> {
        let mut req = self
            .client
            .get(format!("{}/api/monitor/metrics", self.base_url.trim_end_matches('/')));
        if let Some(token) = &self.auth_token {
            if !token.is_empty() {
                req = req.bearer_auth(token);
            }
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ToolError::ExecutionError(format!(
                "Gateway metrics request failed: {}",
                resp.status()
            )));
        }
        let data: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        serde_json::to_string_pretty(&data).map_err(|e| ToolError::ExecutionError(e.to_string()))
    }
}

pub struct DatetimeNowTool;

impl DatetimeNowTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for DatetimeNowTool {
    fn name(&self) -> &str {
        "datetime_now"
    }

    fn description(&self) -> &str {
        "Return current UTC and local-like timestamps."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> Result<String, ToolError> {
        let now = Utc::now();
        let payload = json!({
            "utc_rfc3339": now.to_rfc3339(),
            "utc_epoch_ms": now.timestamp_millis(),
            "utc_date": now.date_naive().to_string(),
            "utc_time": now.format("%H:%M:%S").to_string()
        });
        serde_json::to_string_pretty(&payload).map_err(|e| ToolError::ExecutionError(e.to_string()))
    }
}
