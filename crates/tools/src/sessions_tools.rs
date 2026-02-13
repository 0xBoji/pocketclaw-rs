use crate::{Tool, ToolError};
use async_trait::async_trait;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::types::{Message, Role};
use pocketclaw_persistence::SqliteSessionStore;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct SessionsListTool {
    store: SqliteSessionStore,
}

impl SessionsListTool {
    pub fn new(store: SqliteSessionStore) -> Self {
        Self { store }
    }
}

#[derive(Deserialize)]
struct SessionsListArgs {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    20
}

#[async_trait]
impl Tool for SessionsListTool {
    fn name(&self) -> &str {
        "sessions_list"
    }

    fn description(&self) -> &str {
        "List recent conversation sessions."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum sessions to return (default 20)."
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let args: SessionsListArgs =
            serde_json::from_value(args).unwrap_or(SessionsListArgs { limit: 20 });
        let limit = args.limit.clamp(1, 100);

        let sessions = self
            .store
            .list_sessions(limit)
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        serde_json::to_string_pretty(&sessions)
            .map_err(|e| ToolError::ExecutionError(e.to_string()))
    }
}

pub struct SessionsHistoryTool {
    store: SqliteSessionStore,
}

impl SessionsHistoryTool {
    pub fn new(store: SqliteSessionStore) -> Self {
        Self { store }
    }
}

#[derive(Deserialize)]
struct SessionsHistoryArgs {
    session_key: String,
    #[serde(default = "default_history_limit")]
    limit: i64,
}

fn default_history_limit() -> i64 {
    50
}

#[async_trait]
impl Tool for SessionsHistoryTool {
    fn name(&self) -> &str {
        "sessions_history"
    }

    fn description(&self) -> &str {
        "Fetch message history for a specific session."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_key": {
                    "type": "string",
                    "description": "Session identifier (for example telegram:12345)."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum messages to return (default 50)."
                }
            },
            "required": ["session_key"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let args: SessionsHistoryArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        let limit = args.limit.clamp(1, 200);

        let history = self
            .store
            .get_history(&args.session_key, limit)
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        serde_json::to_string_pretty(&history)
            .map_err(|e| ToolError::ExecutionError(e.to_string()))
    }
}

pub struct SessionsSendTool {
    bus: Arc<MessageBus>,
}

impl SessionsSendTool {
    pub fn new(bus: Arc<MessageBus>) -> Self {
        Self { bus }
    }
}

#[derive(Deserialize)]
struct SessionsSendArgs {
    session_key: String,
    message: String,
    #[serde(default = "default_send_channel")]
    channel: String,
}

fn default_send_channel() -> String {
    "sessions_tool".to_string()
}

#[async_trait]
impl Tool for SessionsSendTool {
    fn name(&self) -> &str {
        "sessions_send"
    }

    fn description(&self) -> &str {
        "Send a user message into an existing session."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "session_key": {
                    "type": "string",
                    "description": "Target session key."
                },
                "message": {
                    "type": "string",
                    "description": "Message text to send."
                },
                "channel": {
                    "type": "string",
                    "description": "Channel label for the injected message."
                }
            },
            "required": ["session_key", "message"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let args: SessionsSendArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let inbound = Message::new(&args.channel, &args.session_key, Role::User, &args.message)
            .with_sender("sessions_send_tool");

        self.bus
            .publish(Event::InboundMessage(inbound))
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        Ok(format!("queued message to {}", args.session_key))
    }
}
