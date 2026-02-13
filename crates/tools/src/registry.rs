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
    const ALLOW_ALL_MARKER: &'static str = "*";

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
        if allowed_tools.iter().any(|a| a == Self::ALLOW_ALL_MARKER) {
            return tools
                .values()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters()
                    })
                })
                .collect();
        }

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
        allowed_tools
            .iter()
            .any(|a| a == tool_name || a == Self::ALLOW_ALL_MARKER)
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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct DummyTool {
        name: &'static str,
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "dummy"
        }

        fn parameters(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(&self, _args: serde_json::Value) -> Result<String, crate::ToolError> {
            Ok("ok".to_string())
        }
    }

    #[test]
    fn empty_allowed_list_denies_all_tools() {
        assert!(!ToolRegistry::is_tool_allowed("read_file", &[]));
    }

    #[test]
    fn wildcard_allows_any_tool() {
        assert!(ToolRegistry::is_tool_allowed("read_file", &["*".to_string()]));
        assert!(ToolRegistry::is_tool_allowed("android_action", &["*".to_string()]));
    }

    #[tokio::test]
    async fn register_and_get_tool() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool { name: "dummy" })).await;

        let tool = registry.get("dummy").await;
        assert!(tool.is_some());
        assert_eq!(tool.expect("tool").name(), "dummy");
    }

    #[tokio::test]
    async fn list_definitions_respects_permission_filter() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool { name: "a" })).await;
        registry.register(Arc::new(DummyTool { name: "b" })).await;

        let allowed = vec!["b".to_string()];
        let defs = registry.list_definitions_for_permissions(&allowed).await;
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0]["name"], "b");
    }

    #[tokio::test]
    async fn list_definitions_returns_all_when_unfiltered() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool { name: "a" })).await;
        registry.register(Arc::new(DummyTool { name: "b" })).await;

        let defs = registry.list_definitions().await;
        assert_eq!(defs.len(), 2);
    }

    #[tokio::test]
    async fn wildcard_permission_returns_all_registered_definitions() {
        let registry = ToolRegistry::new();
        registry.register(Arc::new(DummyTool { name: "a" })).await;
        registry.register(Arc::new(DummyTool { name: "b" })).await;

        let allowed = vec!["*".to_string()];
        let defs = registry.list_definitions_for_permissions(&allowed).await;
        assert_eq!(defs.len(), 2);
    }

    #[tokio::test]
    async fn metrics_are_recorded() {
        let registry = ToolRegistry::new();
        registry.record_metrics("dummy", 40, true).await;
        registry.record_metrics("dummy", 20, false).await;

        let metrics = registry.get_metrics().await;
        let m = metrics.get("dummy").expect("dummy metrics");
        assert_eq!(m.execution_count, 2);
        assert_eq!(m.success_count, 1);
        assert_eq!(m.failure_count, 1);
        assert_eq!(m.total_duration_ms, 60);
    }
}
