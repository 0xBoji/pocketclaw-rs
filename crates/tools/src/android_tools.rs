use crate::{Tool, ToolError};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

/// Trait to abstrat the Android JNI calls.
/// This allows us to mock it for testing or implement it in the mobile-jni crate.
#[async_trait]
pub trait AndroidBridge: Send + Sync {
    async fn click(&self, x: f32, y: f32) -> Result<bool, String>;
    async fn scroll(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> Result<bool, String>;
    async fn back(&self) -> Result<bool, String>;
    async fn home(&self) -> Result<bool, String>;
    async fn input_text(&self, text: String) -> Result<bool, String>;
    async fn dump_hierarchy(&self) -> Result<String, String>;
    async fn screenshot(&self) -> Result<Vec<u8>, String>;
}

// ... AndroidActionTool implementation remains same ...

pub struct AndroidScreenTool {
    bridge: Arc<dyn AndroidBridge>,
}

impl AndroidScreenTool {
    pub fn new(bridge: Arc<dyn AndroidBridge>) -> Self {
        Self { bridge }
    }
}

#[async_trait]
impl Tool for AndroidScreenTool {
    fn name(&self) -> &str {
        "android_screen"
    }

    fn description(&self) -> &str {
        "Interact with the screen content: dump hierarchy (XML) or take a screenshot (PNG)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["dump_hierarchy", "screenshot"],
                    "description": "Action to perform. Defaults to 'dump_hierarchy'.",
                    "default": "dump_hierarchy"
                }
            }
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let action = args["action"].as_str().unwrap_or("dump_hierarchy");
        
        match action {
            "dump_hierarchy" => {
                self.bridge.dump_hierarchy().await.map_err(ToolError::ExecutionError)
            }
            "screenshot" => {
                let bytes = self.bridge.screenshot().await.map_err(ToolError::ExecutionError)?;
                if bytes.is_empty() {
                    return Err(ToolError::ExecutionError("Screenshot returned empty bytes".into()));
                }
                // Return base64 for now, or we could save to file and return path
                // For simplicity to agent, let's return a summary message + maybe base64 snippet?
                // Or better: write to a file in the workspace and return the path relative to workspace.
                // But this tool struct doesn't know about workspace path easily without refactoring.
                // We'll return base64 string for direct usage in multi-modal models.
                use base64::{Engine as _, engine::general_purpose};
                let b64 = general_purpose::STANDARD.encode(&bytes);
                Ok(format!("Screenshot taken. Size: {} bytes. Base64: {}", bytes.len(), b64))
            }
            _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", action))),
        }
    }
}

pub struct AndroidActionTool {
    bridge: Arc<dyn AndroidBridge>,
}

impl AndroidActionTool {
    pub fn new(bridge: Arc<dyn AndroidBridge>) -> Self {
        Self { bridge }
    }
}

#[async_trait]
impl Tool for AndroidActionTool {
    fn name(&self) -> &str {
        "android_action"
    }

    fn description(&self) -> &str {
        "Perform actions on the Android device (click, scroll, back, home, input_text)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["click", "scroll", "back", "home", "input_text"],
                    "description": "The action to perform."
                },
                "x": { "type": "number", "description": "X coordinate (for click)" },
                "y": { "type": "number", "description": "Y coordinate (for click)" },
                "x1": { "type": "number", "description": "Start X (for scroll)" },
                "y1": { "type": "number", "description": "Start Y (for scroll)" },
                "x2": { "type": "number", "description": "End X (for scroll)" },
                "y2": { "type": "number", "description": "End Y (for scroll)" },
                "text": { "type": "string", "description": "Text to input (for input_text)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String, ToolError> {
        let action = args["action"].as_str().ok_or(ToolError::InvalidArgs("action required".into()))?;

        let result = match action {
            "click" => {
                let x = args["x"].as_f64().ok_or(ToolError::InvalidArgs("x required".into()))? as f32;
                let y = args["y"].as_f64().ok_or(ToolError::InvalidArgs("y required".into()))? as f32;
                self.bridge.click(x, y).await
            }
            "scroll" => {
                let x1 = args["x1"].as_f64().ok_or(ToolError::InvalidArgs("x1 required".into()))? as f32;
                let y1 = args["y1"].as_f64().ok_or(ToolError::InvalidArgs("y1 required".into()))? as f32;
                let x2 = args["x2"].as_f64().ok_or(ToolError::InvalidArgs("x2 required".into()))? as f32;
                let y2 = args["y2"].as_f64().ok_or(ToolError::InvalidArgs("y2 required".into()))? as f32;
                self.bridge.scroll(x1, y1, x2, y2).await
            }
            "back" => self.bridge.back().await,
            "home" => self.bridge.home().await,
            "input_text" => {
                let text = args["text"].as_str().ok_or(ToolError::InvalidArgs("text required".into()))?;
                self.bridge.input_text(text.to_string()).await
            }
            _ => return Err(ToolError::InvalidArgs(format!("Unknown action: {}", action))),
        };

        match result {
            Ok(true) => Ok(format!("Action '{}' performed successfully", action)),
            Ok(false) => Err(ToolError::ExecutionError(format!("Action '{}' failed (service returned false)", action))),
            Err(e) => Err(ToolError::ExecutionError(format!("Action '{}' error: {}", action, e))),
        }
    }
}

