use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::types::{Message, Role};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};
use uuid::Uuid;
use axum::extract::{Multipart, DefaultBodyLimit};
use pocketclaw_core::attachment::AttachmentPolicy;
use pocketclaw_core::types::{Attachment, AttachmentKind};
use tokio::io::AsyncWriteExt;
use std::path::{Path, PathBuf};

#[derive(Clone)]
struct AppState {
    bus: Arc<MessageBus>,
    /// If set, all mutating endpoints require `Authorization: Bearer <token>`
    auth_token: Option<String>,
    attachment_policy: AttachmentPolicy,
}

pub struct Gateway {
    bus: Arc<MessageBus>,
    port: u16,
    /// Optional auth token. If None, gateway binds to 127.0.0.1 only.
    auth_token: Option<String>,
    attachment_policy: AttachmentPolicy,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct StatusResponse {
    status: &'static str,
    version: &'static str,
    uptime: &'static str,
}

#[derive(Deserialize)]
struct SendMessageRequest {
    message: String,
    #[serde(default = "default_session_key")]
    session_key: String,
}

fn default_session_key() -> String {
    format!("http:{}", Uuid::new_v4())
}

#[derive(Serialize)]
struct SendMessageResponse {
    id: String,
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl Gateway {
    pub fn new(bus: Arc<MessageBus>, port: u16) -> Self {
        Self {
            bus,
            port,
            auth_token: None,
            attachment_policy: AttachmentPolicy::default(),
        }
    }

    /// Create gateway with auth token. If token is set, binds to 0.0.0.0.
    /// If no token, binds to 127.0.0.1 (local-only) for safety.
    pub fn with_auth(bus: Arc<MessageBus>, port: u16, auth_token: Option<String>) -> Self {
        Self {
            bus,
            port,
            auth_token,
            attachment_policy: AttachmentPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: AttachmentPolicy) -> Self {
        self.attachment_policy = policy;
        self
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let state = AppState {
            bus: self.bus.clone(),
            auth_token: self.auth_token.clone(),
            attachment_policy: self.attachment_policy.clone(),
        };

        // Determine max body size from policy or default
        let max_size = if self.attachment_policy.enabled {
             self.attachment_policy.max_size_bytes
        } else {
             1024 * 1024 // 1MB default if disabled
        };

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/status", get(api_status))
            .route("/api/message", post(send_message))
            .route("/api/attachment", post(upload_attachment))
            .layer(DefaultBodyLimit::max(max_size))
            .with_state(state);

        // Security: bind to localhost-only if no auth token configured
        let addr = if self.auth_token.is_some() {
            SocketAddr::from(([0, 0, 0, 0], self.port))
        } else {
            warn!("No gateway auth token configured — binding to 127.0.0.1 only");
            SocketAddr::from(([127, 0, 0, 1], self.port))
        };

        info!("Gateway listening on {}", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

/// Verify the Authorization header against the configured token.
fn check_auth(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected_token) = &state.auth_token else {
        // No auth configured = local-only, all requests allowed
        return Ok(());
    };

    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let provided_token = auth_header.strip_prefix("Bearer ").unwrap_or("");

    if provided_token == expected_token {
        Ok(())
    } else {
        warn!("Unauthorized gateway access attempt");
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: "0.1.0",
    })
}

async fn api_status(State(_state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "running",
        version: "0.1.0",
        uptime: "N/A",
    })
}

/// POST /api/message — send a message to the agent via HTTP
async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    // Auth check
    check_auth(&state, &headers)?;

    let msg_id = Uuid::new_v4();

    let msg = Message::new(
        "http",
        &req.session_key,
        Role::User,
        &req.message,
    ).with_sender("http_client");

    state
        .bus
        .publish(Event::InboundMessage(msg))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SendMessageResponse {
        id: msg_id.to_string(),
        status: "accepted",
    }))
}

#[derive(Serialize)]
struct UploadResponse {
    id: String,
    url: String,
    filename: String,
    mime_type: String,
    size_bytes: usize,
}

/// POST /api/attachment
async fn upload_attachment(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    // Auth check
    if let Err(code) = check_auth(&state, &headers) {
        return Err((code, "Unauthorized".to_string()));
    }

    // Policy check
    if !state.attachment_policy.enabled {
        return Err((StatusCode::FORBIDDEN, "Attachments are disabled".to_string()));
    }

    // Only process the first field
    while let Some(mut field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let filename = field.file_name().unwrap_or("unknown.bin").to_string();
        // Determine storage path (relative to CWD for now, or assume configured properly)
        // Ideally should be absolute path from config.
        let storage_dir = &state.attachment_policy.storage_directory;

        if !storage_dir.exists() {
             tokio::fs::create_dir_all(storage_dir).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }

        let id = Uuid::new_v4();
        let file_path = storage_dir.join(id.to_string());
        let mut file = tokio::fs::File::create(&file_path).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut size = 0;
        let mut first_chunk = true;
        let mut detected_mime = None;

        while let Some(chunk) = field.chunk().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
            let len = chunk.len();
            if size + len > state.attachment_policy.max_size_bytes {
                drop(file);
                let _ = tokio::fs::remove_file(&file_path).await;
                return Err((StatusCode::PAYLOAD_TOO_LARGE, "File too large".to_string()));
            }

            if first_chunk {
                // Detect mime type from first chunk
                if let Some(kind) = infer::get(&chunk) {
                     let mime = kind.mime_type();
                     if !state.attachment_policy.allowed_mime_types.contains(&mime.to_string()) {
                          drop(file);
                          let _ = tokio::fs::remove_file(&file_path).await;
                          return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, format!("MIME type {} not allowed", mime)));
                     }
                     detected_mime = Some(mime.to_string());
                } else {
                     drop(file);
                     let _ = tokio::fs::remove_file(&file_path).await;
                     return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, "Could not detect MIME type".to_string()));
                }
                first_chunk = false;
            }

            file.write_all(&chunk).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            size += len;
        }

        // If file was empty, first_chunk is still true
        if first_chunk {
             drop(file);
             let _ = tokio::fs::remove_file(&file_path).await;
             return Err((StatusCode::BAD_REQUEST, "Empty file".to_string()));
        }

        return Ok(Json(UploadResponse {
            id: id.to_string(),
            url: format!("attachment://{}", id),
            filename,
            mime_type: detected_mime.unwrap_or_default(),
            size_bytes: size,
        }));
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))
}
