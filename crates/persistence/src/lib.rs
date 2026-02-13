use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use pocketclaw_core::types::{Message, Role};
use serde::Serialize;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tracing::{info, instrument};
use uuid::Uuid;

#[derive(Clone)]
pub struct SqliteSessionStore {
    pool: SqlitePool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub session_key: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i64,
}

impl SqliteSessionStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .context("Failed to connect to SQLite database")?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("Failed to run migrations")?;

        info!("SqliteSessionStore initialized");
        Ok(Self { pool })
    }

    #[instrument(skip(self))]
    pub async fn ensure_session(&self, session_key: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO sessions (id, title)
            VALUES (?, ?)
            "#,
        )
        .bind(session_key)
        .bind(format!("Session {}", session_key))
        .execute(&self.pool)
        .await
        .context("Failed to ensure session")?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn add_message(&self, msg: &Message) -> Result<()> {
        self.ensure_session(&msg.session_key).await?;

        let metadata_json = serde_json::to_value(&msg.metadata)?;

        sqlx::query(
            r#"
            INSERT INTO messages (id, session_id, sender_id, role, content, metadata, created_at, reply_to)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(msg.id.to_string())
        .bind(&msg.session_key)
        .bind(&msg.sender_id)
        .bind(format!("{:?}", msg.role).to_lowercase())
        .bind(&msg.content)
        .bind(metadata_json)
        .bind(msg.created_at)
        .bind(msg.reply_to.map(|id| id.to_string()))
        .execute(&self.pool)
        .await
        .context("Failed to insert message")?;

        // Update session updated_at
        sqlx::query(
            r#"
            UPDATE sessions SET updated_at = CURRENT_TIMESTAMP WHERE id = ?
            "#,
        )
        .bind(&msg.session_key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_history(&self, session_key: &str, limit: i64) -> Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, serde_json::Value, DateTime<Utc>, Option<String>)>(
            r#"
            SELECT id, session_id, sender_id, role, content, metadata, created_at, reply_to
            FROM messages
            WHERE session_id = ?
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(session_key)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch history")?;

        let mut history = Vec::new();
        for (id, session_id, sender_id, role_str, content, metadata_val, created_at, reply_to_str) in rows {
            let role = match role_str.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => Role::User,
            };

            let metadata: std::collections::HashMap<String, String> = serde_json::from_value(metadata_val).unwrap_or_default();
            let reply_to = reply_to_str.and_then(|s| Uuid::parse_str(&s).ok());

            history.push(Message {
                id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::new_v4()),
                channel: "sqlite".to_string(), // Store doesn't verify channel origin, maybe separate field in DB?
                session_key: session_id, // This is the session_key
                sender_id,
                content,
                role,
                created_at,
                reply_to,
                attachments: Vec::new(), // Not stored in DB yet (Wave B1 limit)
                metadata,
            });
        }
        Ok(history)
    }

    #[instrument(skip(self))]
    pub async fn get_summary(&self, session_key: &str) -> Result<Option<String>> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT summary FROM sessions WHERE id = ?",
        )
        .bind(session_key)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get summary")?;

        Ok(row.and_then(|r| r.0))
    }

    #[instrument(skip(self))]
    pub async fn set_summary(&self, session_key: &str, summary: String) -> Result<()> {
        self.ensure_session(session_key).await?;

        sqlx::query("UPDATE sessions SET summary = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(summary)
            .bind(session_key)
            .execute(&self.pool)
            .await
            .context("Failed to set summary")?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn trim_history(&self, session_key: &str, keep: i64) -> Result<u64> {
        // SQLite doesn't support DELETE ... LIMIT directly in standard SQL without compilation options,
        // but supports subqueries.
        // Delete messages that are NOT in the top N recent messages.
        let result = sqlx::query(
            r#"
            DELETE FROM messages 
            WHERE session_id = ? 
            AND id NOT IN (
                SELECT id FROM messages 
                WHERE session_id = ? 
                ORDER BY created_at DESC 
                LIMIT ?
            )
            "#,
        )
        .bind(session_key)
        .bind(session_key)
        .bind(keep)
        .execute(&self.pool)
        .await
        .context("Failed to trim history")?;
        
        Ok(result.rows_affected())
    }

    #[instrument(skip(self))]
    pub async fn list_sessions(&self, limit: i64) -> Result<Vec<SessionInfo>> {
        let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, DateTime<Utc>, DateTime<Utc>, i64)>(
            r#"
            SELECT
                s.id,
                s.title,
                s.summary,
                s.created_at,
                s.updated_at,
                COUNT(m.id) as message_count
            FROM sessions s
            LEFT JOIN messages m ON m.session_id = s.id
            GROUP BY s.id
            ORDER BY s.updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list sessions")?;

        Ok(rows
            .into_iter()
            .map(
                |(session_key, title, summary, created_at, updated_at, message_count)| SessionInfo {
                    session_key,
                    title,
                    summary,
                    created_at,
                    updated_at,
                    message_count,
                },
            )
            .collect())
    }
}
