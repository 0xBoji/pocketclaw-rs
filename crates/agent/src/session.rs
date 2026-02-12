use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use pocketclaw_core::types::Message;
use pocketclaw_persistence::SqliteSessionStore;
use crate::sheets::SheetsClient;
use tracing::{error, info};

#[derive(Clone)]
pub struct SessionManager {
    store: SqliteSessionStore,
    sheets_client: Option<SheetsClient>,
    last_summary: Arc<RwLock<HashMap<String, Instant>>>,
}

impl SessionManager {
    pub fn new(store: SqliteSessionStore, sheets_client: Option<SheetsClient>) -> Self {
        Self { 
            store, 
            sheets_client,
            last_summary: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_history(&self, session_key: &str) -> Vec<Message> {
        // Fetch up to 100 messages for context
        match self.store.get_history(session_key, 100).await {
            Ok(history) => history,
            Err(e) => {
                error!("Failed to load session history for {}: {}", session_key, e);
                Vec::new()
            }
        }
    }

    pub async fn add_message(&self, session_key: &str, message: Message) {
        // Persist to SQLite
        if let Err(e) = self.store.add_message(&message).await {
            error!("Failed to save message to SQLite: {}", e);
        }

        // Persist to sheets (append only)
        if let Some(client) = &self.sheets_client {
            let client = client.clone();
            let session_key = session_key.to_string();
            let msg = message.clone();
            tokio::spawn(async move {
                if let Err(e) = client.append_message(&session_key, &msg).await {
                    error!("Failed to append to Google Sheets: {}", e);
                }
            });
        }
    }

    pub async fn get_summary(&self, session_key: &str) -> Option<String> {
        match self.store.get_summary(session_key).await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to load summary: {}", e);
                None
            }
        }
    }

    pub async fn set_summary(&self, session_key: &str, summary: String) {
        if let Err(e) = self.store.set_summary(session_key, summary).await {
            error!("Failed to save summary: {}", e);
        }
    }

    pub fn should_summarize(&self, session_key: &str, history_len: usize) -> bool {
        if history_len < 30 {
            return false;
        }

        let last = {
            let map = self.last_summary.read().unwrap();
            map.get(session_key).copied()
        };

        if let Some(last_time) = last {
            if last_time.elapsed() < Duration::from_secs(300) {
                return false;
            }
        }
        
        true
    }

    pub fn mark_summarized(&self, session_key: &str) {
        let mut map = self.last_summary.write().unwrap();
        map.insert(session_key.to_string(), Instant::now());
    }

    /// Trim history in the database, keeping only the most recent `keep` messages.
    pub async fn auto_trim_history(&self, session_key: &str, keep: usize) {
        match self.store.trim_history(session_key, keep as i64).await {
            Ok(deleted) => {
                if deleted > 0 {
                    info!(session = %session_key, deleted = %deleted, "Trimmed session history");
                }
            }
            Err(e) => {
                error!("Failed to trim history: {}", e);
            }
        }
    }
}
