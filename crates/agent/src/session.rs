use pocketclaw_core::types::Message;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Session {
    pub history: Vec<Message>,
    pub summary: Option<String>,
}

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    storage_path: PathBuf,
}

impl SessionManager {
    pub fn new(workspace: PathBuf) -> Self {
        let storage_path = workspace.join("sessions");
        
        // Ensure sessions directory exists
        // Note: active waiting for async in new() is not ideal, but acceptable for initialization
        // Better: Make new() async or have an init() method. 
        // For simplicity in this architecture, we'll assume the caller ensures the directory or we do it lazily.
        
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            storage_path,
        }
    }

    async fn load_session(&self, session_key: &str) -> Session {
        let safe_key = session_key.replace(":", "_");
        let file_path = self.storage_path.join(format!("{}.json", safe_key));
        
        if file_path.exists() {
             if let Ok(content) = fs::read_to_string(&file_path).await {
                 if let Ok(session) = serde_json::from_str(&content) {
                     return session;
                 }
             }
        }
        Session::default()
    }
    
    async fn save_session(&self, session_key: &str, session: &Session) {
        if !self.storage_path.exists() {
             let _ = fs::create_dir_all(&self.storage_path).await;
        }

        let safe_key = session_key.replace(":", "_");
        let file_path = self.storage_path.join(format!("{}.json", safe_key));
        
        if let Ok(content) = serde_json::to_string_pretty(session) {
            let _ = fs::write(file_path, content).await;
        }
    }

    pub async fn get_history(&self, session_key: &str) -> Vec<Message> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get(session_key) {
            return session.history.clone();
        }
        
        // Try load from disk
        let session = self.load_session(session_key).await;
        let history = session.history.clone();
        sessions.insert(session_key.to_string(), session);
        
        history
    }

    pub async fn add_message(&self, session_key: &str, message: Message) {
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(session_key.to_string()).or_insert_with(Session::default);
        session.history.push(message);
        
        // Persist to disk
        // Optimization: In a real app, this should be debounced or done in background
        self.save_session(session_key, session).await;
    }

    pub async fn get_summary(&self, session_key: &str) -> Option<String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get(session_key) {
            return session.summary.clone();
        }

        let session = self.load_session(session_key).await;
        let summary = session.summary.clone();
        sessions.insert(session_key.to_string(), session);
        
        summary
    }

    pub async fn set_summary(&self, session_key: &str, summary: String) {
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(session_key.to_string()).or_insert_with(Session::default);
        session.summary = Some(summary);
        
        self.save_session(session_key, session).await;
    }
}
