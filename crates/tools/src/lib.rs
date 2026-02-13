pub mod web_fetch;
pub mod web_search;
pub mod registry;
pub mod exec_tool;
pub mod fs_tools;
pub mod sandbox;
pub mod sessions_tools;
pub mod platform_tools;
use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value; // JSON Schema
    async fn execute(&self, args: Value) -> Result<String, ToolError>;
}
