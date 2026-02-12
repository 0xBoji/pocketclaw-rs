use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub channel: String,
    pub session_key: String,
    /// ID of the sender (e.g. user ID, tool name, or "system")
    #[serde(default)]
    pub sender_id: String,
    pub content: String,
    pub role: Role,
    /// Creation timestamp
    #[serde(default = "default_timestamp")]
    pub created_at: DateTime<Utc>,
    /// ID of the message this is replying to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Uuid>,
    /// File attachments (images, docs, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    pub metadata: HashMap<String, String>,
}

fn default_timestamp() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    pub kind: AttachmentKind,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    File,
    Audio,
    Video,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Message {
    pub fn new(channel: &str, session_key: &str, role: Role, content: &str) -> Self {
        // Infer sender_id from role/channel if possible, else default to "unknown"
        let sender_id = match role {
            Role::System => "system".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::Tool => "tool".to_string(),
            Role::User => "user".to_string(), // Callers should override this if they know the ID
        };

        Self {
            id: Uuid::new_v4(),
            channel: channel.to_string(),
            session_key: session_key.to_string(),
            sender_id,
            content: content.to_string(),
            role,
            created_at: Utc::now(),
            reply_to: None,
            attachments: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Builder-style method to set sender_id
    pub fn with_sender(mut self, sender_id: &str) -> Self {
        self.sender_id = sender_id.to_string();
        self
    }
    
    /// Builder-style method to set reply_to
    pub fn reply_to(mut self, msg_id: Uuid) -> Self {
        self.reply_to = Some(msg_id);
        self
    }
    
    /// Builder-style method to add attachment
    pub fn add_attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }
}
