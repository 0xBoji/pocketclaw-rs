use serde::Serialize;
use serde_json::Value;
use std::time::SystemTime;
use tracing::info;

#[derive(Serialize)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub event_type: String,
    pub session_key: String,
    pub details: Value,
}

pub fn log_audit_internal(event_type: &str, session_key: &str, details: Value) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let event = AuditEvent {
        timestamp: now,
        event_type: event_type.to_string(),
        session_key: session_key.to_string(),
        details,
    };

    // Serialize to JSON string immediately to ensure the log payload is clean JSON
    // The subscriber will receive this string as the message.
    if let Ok(json_str) = serde_json::to_string(&event) {
        info!(target: "audit", "{}", json_str);
    }
}
