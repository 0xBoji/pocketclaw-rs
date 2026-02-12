use crate::Tool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-tool execution metrics.
#[derive(Debug, Clone, Default)]
pub struct ToolMetrics {
    pub execution_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub total_duration_ms: u64,
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
    metrics: Arc<RwLock<HashMap<String, ToolMetrics>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write().await;
        tools.insert(tool.name().to_string(), tool);
    }

    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    pub async fn list_definitions(&self) -> Vec<serde_json::Value> {
        let tools = self.tools.read().await;
        tools.values().map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "parameters": t.parameters()
            })
        }).collect()
    }

    /// Return tool definitions filtered by an allowed-tools list.
    /// If `allowed_tools` is empty, returns NO tools (strict default deny).
    pub async fn list_definitions_for_permissions(
        &self,
        allowed_tools: &[String],
    ) -> Vec<serde_json::Value> {
        // Strict Mode: Empty allowed list means NOTHING is allowed.
        if allowed_tools.is_empty() {
            return Vec::new();
        }

        let tools = self.tools.read().await;
        tools.values()
            .filter(|t| allowed_tools.iter().any(|a| a == t.name()))
            .map(|t| {
                serde_json::json!({
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": t.parameters()
                })
            })
            .collect()
    }

    /// Check if a tool name is in the allowed list.
    /// If `allowed_tools` is empty, NO tools are allowed.
    pub fn is_tool_allowed(tool_name: &str, allowed_tools: &[String]) -> bool {
        allowed_tools.iter().any(|a| a == tool_name)
    }

    /// Record metrics for a tool execution.
    pub async fn record_metrics(&self, tool_name: &str, duration_ms: u64, success: bool) {
        let mut metrics = self.metrics.write().await;
        let entry = metrics.entry(tool_name.to_string()).or_default();
        entry.execution_count += 1;
        entry.total_duration_ms += duration_ms;
        if success {
            entry.success_count += 1;
        } else {
            entry.failure_count += 1;
        }
    }

    /// Get metrics for all tools.
    pub async fn get_metrics(&self) -> HashMap<String, ToolMetrics> {
        self.metrics.read().await.clone()
    }
}
