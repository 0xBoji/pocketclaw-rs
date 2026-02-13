use axum::{
    body::Bytes,
    extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use pocketclaw_core::metrics::{MetricsStore, MetricsSnapshot};
use tokio::sync::mpsc;
use pocketclaw_core::bus::{Event, MessageBus};
use pocketclaw_core::channel::{target_personal_channels, native_supported_channels};
use pocketclaw_core::types::{Message, Role};
use pocketclaw_persistence::SqliteSessionStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::time::{interval, Duration};
use tracing::{info, warn};
use uuid::Uuid;
use axum::extract::{Multipart, DefaultBodyLimit};
use pocketclaw_core::attachment::AttachmentPolicy;
use tokio::sync::broadcast::error::RecvError;
use tokio::io::AsyncWriteExt;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const ROLLING_WINDOW_MINUTES: usize = 60;

#[derive(Debug, Clone, Copy)]
pub struct GatewayRuntimeConfig {
    pub ws_heartbeat_secs: u64,
    pub health_window_minutes: usize,
    pub dedupe_max_entries: usize,
}

impl Default for GatewayRuntimeConfig {
    fn default() -> Self {
        Self {
            ws_heartbeat_secs: 15,
            health_window_minutes: 60,
            dedupe_max_entries: 2048,
        }
    }
}

#[derive(Debug, Clone)]
struct RollingMinuteCounter {
    buckets: [u32; ROLLING_WINDOW_MINUTES],
    cursor: usize,
    last_minute: i64,
}

impl Default for RollingMinuteCounter {
    fn default() -> Self {
        Self {
            buckets: [0; ROLLING_WINDOW_MINUTES],
            cursor: 0,
            last_minute: 0,
        }
    }
}

impl RollingMinuteCounter {
    fn minute_index(epoch_ms: i64) -> i64 {
        epoch_ms / 60_000
    }

    fn advance_to(&mut self, now_ms: i64) {
        let now_minute = Self::minute_index(now_ms);
        if self.last_minute == 0 {
            self.last_minute = now_minute;
            return;
        }

        let delta = now_minute - self.last_minute;
        if delta <= 0 {
            return;
        }

        if delta as usize >= ROLLING_WINDOW_MINUTES {
            self.buckets = [0; ROLLING_WINDOW_MINUTES];
            self.cursor = 0;
        } else {
            for _ in 0..delta {
                self.cursor = (self.cursor + 1) % ROLLING_WINDOW_MINUTES;
                self.buckets[self.cursor] = 0;
            }
        }

        self.last_minute = now_minute;
    }

    fn observe(&mut self, now_ms: i64) {
        self.advance_to(now_ms);
        self.buckets[self.cursor] = self.buckets[self.cursor].saturating_add(1);
    }

    fn sum_recent_minutes(&mut self, now_ms: i64, minutes: usize) -> u64 {
        self.advance_to(now_ms);
        let take = minutes.clamp(1, ROLLING_WINDOW_MINUTES);
        (0..take)
            .map(|offset| {
                let idx = (self.cursor + ROLLING_WINDOW_MINUTES - offset) % ROLLING_WINDOW_MINUTES;
                self.buckets[idx] as u64
            })
            .sum()
    }
}

#[derive(Debug, Default)]
struct DedupeCache {
    entries: HashMap<String, i64>,
    order: VecDeque<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ChannelRuntimeStats {
    last_inbound_at_ms: Option<i64>,
    last_outbound_at_ms: Option<i64>,
    error_count: u64,
    last_error: Option<String>,
    last_error_at_ms: Option<i64>,
    #[serde(skip)]
    inbound_rolling_1h: RollingMinuteCounter,
    #[serde(skip)]
    outbound_rolling_1h: RollingMinuteCounter,
    #[serde(skip)]
    error_rolling_1h: RollingMinuteCounter,
}

#[derive(Clone)]
struct AppState {
    bus: Arc<MessageBus>,
    /// If set, all mutating endpoints require `Authorization: Bearer <token>`
    auth_token: Option<String>,
    attachment_policy: AttachmentPolicy,
    metrics: Arc<MetricsStore>,
    reload_tx: mpsc::Sender<()>,
    sessions: SqliteSessionStore,
    whatsapp_verify_token: Option<String>,
    whatsapp_app_secret: Option<String>,
    slack_signing_secret: Option<String>,
    configured_channels: Vec<String>,
    runtime: GatewayRuntimeConfig,
    channel_stats: Arc<tokio::sync::Mutex<HashMap<String, ChannelRuntimeStats>>>,
    dedupe_cache: Arc<tokio::sync::Mutex<DedupeCache>>,
}

pub struct Gateway {
    bus: Arc<MessageBus>,
    port: u16,
    /// Optional auth token. If None, gateway binds to 127.0.0.1 only.
    auth_token: Option<String>,
    attachment_policy: AttachmentPolicy,
    metrics: Arc<MetricsStore>,
    reload_tx: mpsc::Sender<()>,
    sessions: SqliteSessionStore,
    whatsapp_verify_token: Option<String>,
    whatsapp_app_secret: Option<String>,
    slack_signing_secret: Option<String>,
    configured_channels: Vec<String>,
    runtime: GatewayRuntimeConfig,
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
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

impl Gateway {
    pub fn new(
        bus: Arc<MessageBus>,
        port: u16,
        metrics: Arc<MetricsStore>,
        reload_tx: mpsc::Sender<()>,
        sessions: SqliteSessionStore,
        whatsapp_verify_token: Option<String>,
        whatsapp_app_secret: Option<String>,
        slack_signing_secret: Option<String>,
        configured_channels: Vec<String>,
        runtime: GatewayRuntimeConfig,
    ) -> Self {
        Self {
            bus,
            port,
            auth_token: None,
            attachment_policy: AttachmentPolicy::default(),
            metrics,
            reload_tx,
            sessions,
            whatsapp_verify_token,
            whatsapp_app_secret,
            slack_signing_secret,
            configured_channels,
            runtime,
        }
    }

    /// Create gateway with auth token. If token is set, binds to 0.0.0.0.
    /// If no token, binds to 127.0.0.1 (local-only) for safety.
    pub fn with_auth(
        bus: Arc<MessageBus>,
        port: u16,
        auth_token: Option<String>,
        metrics: Arc<MetricsStore>,
        reload_tx: mpsc::Sender<()>,
        sessions: SqliteSessionStore,
        whatsapp_verify_token: Option<String>,
        whatsapp_app_secret: Option<String>,
        slack_signing_secret: Option<String>,
        configured_channels: Vec<String>,
        runtime: GatewayRuntimeConfig,
    ) -> Self {
        Self {
            bus,
            port,
            auth_token,
            attachment_policy: AttachmentPolicy::default(),
            metrics,
            reload_tx,
            sessions,
            whatsapp_verify_token,
            whatsapp_app_secret,
            slack_signing_secret,
            configured_channels,
            runtime,
        }
    }

    pub fn with_policy(mut self, policy: AttachmentPolicy) -> Self {
        self.attachment_policy = policy;
        self
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let channel_stats: Arc<tokio::sync::Mutex<HashMap<String, ChannelRuntimeStats>>> =
            Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let dedupe_cache = Arc::new(tokio::sync::Mutex::new(DedupeCache::default()));

        // Monitor bus events to keep channel runtime health fresh.
        {
            let stats = channel_stats.clone();
            let mut rx = self.bus.subscribe();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(Event::InboundMessage(msg)) => {
                            let channel = resolve_channel_from_message(&msg);
                            let mut lock = stats.lock().await;
                            let entry = lock.entry(channel).or_default();
                            let now = now_ms_epoch();
                            entry.last_inbound_at_ms = Some(now);
                            entry.inbound_rolling_1h.observe(now);
                        }
                        Ok(Event::OutboundMessage(msg)) => {
                            let channel = resolve_channel_from_message(&msg);
                            let mut lock = stats.lock().await;
                            let entry = lock.entry(channel).or_default();
                            let now = now_ms_epoch();
                            entry.last_outbound_at_ms = Some(now);
                            entry.outbound_rolling_1h.observe(now);
                        }
                        Ok(Event::SystemLog { .. }) => {}
                        Err(RecvError::Lagged(skipped)) => {
                            let mut lock = stats.lock().await;
                            let entry = lock.entry("gateway".to_string()).or_default();
                            let now = now_ms_epoch();
                            entry.error_count += 1;
                            entry.last_error = Some(format!("bus_lagged: {}", skipped));
                            entry.last_error_at_ms = Some(now);
                            entry.error_rolling_1h.observe(now);
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            });
        }

        let state = AppState {
            bus: self.bus.clone(),
            auth_token: self.auth_token.clone(),
            attachment_policy: self.attachment_policy.clone(),
            metrics: self.metrics.clone(),
            reload_tx: self.reload_tx.clone(),
            sessions: self.sessions.clone(),
            whatsapp_verify_token: self.whatsapp_verify_token.clone(),
            whatsapp_app_secret: self.whatsapp_app_secret.clone(),
            slack_signing_secret: self.slack_signing_secret.clone(),
            configured_channels: self.configured_channels.clone(),
            runtime: self.runtime,
            channel_stats,
            dedupe_cache,
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
            .route("/api/control/reload", put(reload_config))
            .route("/api/monitor/metrics", get(get_metrics))
            .route("/api/sessions", get(list_sessions))
            .route("/api/sessions/:session_key/messages", get(get_session_messages))
            .route("/api/sessions/send", post(send_session_message))
            .route("/api/channels/whatsapp/inbound", post(whatsapp_inbound))
            .route("/api/channels/whatsapp/webhook", get(whatsapp_verify))
            .route("/api/channels/whatsapp/webhook", post(whatsapp_webhook))
            .route("/api/channels/slack/inbound", post(slack_inbound))
            .route("/api/channels/googlechat/inbound", post(google_chat_inbound))
            .route("/api/channels/zalo/inbound", post(zalo_inbound))
            .route("/api/channels/teams/inbound", post(teams_inbound))
            .route("/api/channels/health", get(channel_health))
            .route("/ws/events", get(ws_events))
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

fn now_ms_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn resolve_channel_from_message(msg: &Message) -> String {
    if let Some((prefix, _rest)) = msg.session_key.split_once(':') {
        let known = target_personal_channels();
        if known.contains(&prefix) {
            return prefix.to_string();
        }
    }
    msg.channel.clone()
}

async fn record_channel_error(state: &AppState, channel: &str, error: impl Into<String>) {
    let mut stats = state.channel_stats.lock().await;
    let entry = stats.entry(channel.to_string()).or_default();
    let now = now_ms_epoch();
    entry.error_count += 1;
    entry.last_error = Some(error.into());
    entry.last_error_at_ms = Some(now);
    entry.error_rolling_1h.observe(now);
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

async fn reload_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;
    info!("Control request: Config reload triggered");
    state.reload_tx.send(()).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "status": "reload_triggered" })))
}

async fn get_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MetricsSnapshot>, StatusCode> {
    check_auth(&state, &headers)?;
    Ok(Json(state.metrics.snapshot()))
}

async fn channel_health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;

    let configured = state.configured_channels.clone();
    let native = native_supported_channels()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    let now_ms = now_ms_epoch();
    let mut stats_lock = state.channel_stats.lock().await;
    let window_minutes = state
        .runtime
        .health_window_minutes
        .clamp(1, ROLLING_WINDOW_MINUTES);

    let channels = target_personal_channels()
        .iter()
        .map(|ch| {
            let name = (*ch).to_string();
            let is_configured = configured.iter().any(|c| c == ch);
            let is_native = native.iter().any(|c| c == ch);
            let runtime = stats_lock.entry(name.clone()).or_default();
            let inbound_1h = runtime
                .inbound_rolling_1h
                .sum_recent_minutes(now_ms, window_minutes);
            let outbound_1h = runtime
                .outbound_rolling_1h
                .sum_recent_minutes(now_ms, window_minutes);
            let errors_1h = runtime
                .error_rolling_1h
                .sum_recent_minutes(now_ms, window_minutes);
            let stability = if errors_1h >= 5 {
                "unstable"
            } else if errors_1h > 0 {
                "degraded"
            } else if inbound_1h + outbound_1h == 0 {
                "idle"
            } else {
                "healthy"
            };
            let adapter_status = if is_configured && is_native {
                "running"
            } else if is_configured && !is_native {
                "configured_pending_adapter"
            } else {
                "disabled"
            };
            json!({
                "channel": name,
                "configured": is_configured,
                "native_supported": is_native,
                "status": adapter_status,
                "last_inbound_at_ms": runtime.last_inbound_at_ms,
                "last_outbound_at_ms": runtime.last_outbound_at_ms,
                "error_count": runtime.error_count,
                "last_error": runtime.last_error,
                "last_error_at_ms": runtime.last_error_at_ms,
                "trend_1h": {
                    "window_minutes": window_minutes,
                    "inbound_count": inbound_1h,
                    "outbound_count": outbound_1h,
                    "error_count": errors_1h,
                    "stability": stability
                }
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "channels": channels,
        "configured_count": configured.len(),
        "native_supported_count": native.len(),
    })))
}

async fn ws_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, StatusCode> {
    check_auth(&state, &headers)?;

    let bus = state.bus.clone();
    let metrics = state.metrics.clone();
    let ws_heartbeat_secs = state.runtime.ws_heartbeat_secs.max(3);
    Ok(ws.on_upgrade(move |socket| handle_ws_events(socket, bus, metrics, ws_heartbeat_secs)))
}

#[derive(Deserialize)]
struct SessionListQuery {
    #[serde(default = "default_sessions_limit")]
    limit: i64,
}

fn default_sessions_limit() -> i64 {
    20
}

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SessionListQuery>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;
    let limit = query.limit.clamp(1, 100);
    let sessions = state
        .sessions
        .list_sessions(limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "sessions": sessions })))
}

#[derive(Deserialize)]
struct SessionMessagesQuery {
    #[serde(default = "default_messages_limit")]
    limit: i64,
}

fn default_messages_limit() -> i64 {
    100
}

async fn get_session_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_key): Path<String>,
    Query(query): Query<SessionMessagesQuery>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;
    let limit = query.limit.clamp(1, 500);
    let messages = state
        .sessions
        .get_history(&session_key, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "session_key": session_key, "messages": messages })))
}

#[derive(Deserialize)]
struct SessionSendRequest {
    session_key: String,
    message: String,
    #[serde(default = "default_session_send_channel")]
    channel: String,
}

fn default_session_send_channel() -> String {
    "api.sessions".to_string()
}

async fn send_session_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SessionSendRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;
    let inbound = Message::new(&req.channel, &req.session_key, Role::User, &req.message)
        .with_sender("api_session_send");
    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "status": "accepted", "session_key": req.session_key })))
}

#[derive(Deserialize)]
struct WhatsAppInboundRequest {
    from: String,
    text: String,
    #[serde(default)]
    sender_name: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
}

async fn whatsapp_inbound(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<WhatsAppInboundRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;

    if let Some(message_id) = req.message_id.as_deref() {
        let key = format!("whatsapp:{}", message_id);
        if dedupe_seen(&state, &key, 600).await {
            return Ok(Json(json!({
                "status": "duplicate_ignored",
                "session_key": format!("whatsapp:{}", req.from)
            })));
        }
    }

    let mut inbound = Message::new(
        "whatsapp",
        &format!("whatsapp:{}", req.from),
        Role::User,
        &req.text,
    )
    .with_sender(&req.from);

    if let Some(name) = req.sender_name {
        inbound.metadata.insert("sender_name".to_string(), name);
    }
    if let Some(message_id) = req.message_id {
        inbound.metadata.insert("whatsapp_message_id".to_string(), message_id);
    }

    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "status": "accepted",
        "session_key": format!("whatsapp:{}", req.from)
    })))
}

#[derive(Deserialize)]
struct WhatsAppVerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

async fn whatsapp_verify(
    State(state): State<AppState>,
    Query(query): Query<WhatsAppVerifyQuery>,
) -> Result<String, StatusCode> {
    let expected = state.whatsapp_verify_token.as_deref().unwrap_or_default();
    let provided = query.verify_token.as_deref().unwrap_or_default();

    if query.mode.as_deref() != Some("subscribe") {
        record_channel_error(&state, "whatsapp", "verify_mode_invalid").await;
        return Err(StatusCode::BAD_REQUEST);
    }
    if expected.is_empty() || provided != expected {
        record_channel_error(&state, "whatsapp", "verify_token_invalid").await;
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(query.challenge.unwrap_or_default())
}

fn now_epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

async fn dedupe_seen(state: &AppState, key: &str, ttl_secs: i64) -> bool {
    let now = now_epoch_secs();
    let mut cache = state.dedupe_cache.lock().await;

    while let Some((old_key, old_ts)) = cache.order.front() {
        if now - *old_ts > ttl_secs {
            let old_key = old_key.clone();
            let old_ts = *old_ts;
            cache.order.pop_front();
            if cache.entries.get(&old_key).is_some_and(|ts| *ts == old_ts) {
                cache.entries.remove(&old_key);
            }
        } else {
            break;
        }
    }

    if let Some(ts) = cache.entries.get(key) {
        if now - *ts <= ttl_secs {
            return true;
        }
    }

    let key = key.to_string();
    cache.entries.insert(key.clone(), now);
    cache.order.push_back((key, now));

    let max_entries = state.runtime.dedupe_max_entries.max(128);
    while cache.entries.len() > max_entries {
        let Some((evict_key, evict_ts)) = cache.order.pop_front() else {
            break;
        };
        if cache
            .entries
            .get(&evict_key)
            .is_some_and(|ts| *ts == evict_ts)
        {
            cache.entries.remove(&evict_key);
        }
    }

    false
}

fn verify_whatsapp_signature(headers: &HeaderMap, body: &[u8], secret: &str) -> bool {
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();

    let Some(sig_hex) = signature.strip_prefix("sha256=") else {
        return false;
    };

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let expected_hex = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    expected_hex == sig_hex.to_ascii_lowercase()
}

async fn whatsapp_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, StatusCode> {
    if let Some(secret) = state.whatsapp_app_secret.as_deref() {
        if !secret.is_empty() && !verify_whatsapp_signature(&headers, &body, secret) {
            record_channel_error(&state, "whatsapp", "signature_invalid").await;
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let payload: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            record_channel_error(&state, "whatsapp", "invalid_json").await;
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    let mut accepted = 0usize;

    let entries = payload
        .get("entry")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for entry in entries {
        let changes = entry
            .get("changes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for change in changes {
            let value = change.get("value").cloned().unwrap_or_else(|| json!({}));
            let messages = value
                .get("messages")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let contacts = value
                .get("contacts")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let sender_name = contacts
                .first()
                .and_then(|c| c.get("profile"))
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(|s| s.to_string());

            for msg in messages {
                let from = msg.get("from").and_then(|v| v.as_str()).unwrap_or_default();
                if from.is_empty() {
                    continue;
                }
                let text = msg
                    .get("text")
                    .and_then(|t| t.get("body"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if text.is_empty() {
                    continue;
                }

                let mut inbound =
                    Message::new("whatsapp", &format!("whatsapp:{}", from), Role::User, &text)
                        .with_sender(from);
                if let Some(name) = &sender_name {
                    inbound
                        .metadata
                        .insert("sender_name".to_string(), name.to_string());
                }
                if let Some(message_id) = msg.get("id").and_then(|v| v.as_str()) {
                    let dedupe_key = format!("whatsapp:{}", message_id);
                    if dedupe_seen(&state, &dedupe_key, 600).await {
                        continue;
                    }
                    inbound.metadata.insert(
                        "whatsapp_message_id".to_string(),
                        message_id.to_string(),
                    );
                }

                state
                    .bus
                    .publish(Event::InboundMessage(inbound))
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                accepted += 1;
            }
        }
    }

    Ok(Json(json!({ "status": "ok", "accepted": accepted })))
}

#[derive(Deserialize)]
struct SlackInboundRequest {
    channel: String,
    text: String,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    thread_ts: Option<String>,
}

#[derive(Deserialize)]
struct GoogleChatInboundRequest {
    space: String,
    text: String,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    thread_key: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
}

#[derive(Deserialize)]
struct ZaloInboundRequest {
    from: String,
    text: String,
    #[serde(default)]
    sender_name: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
}

#[derive(Deserialize)]
struct TeamsInboundRequest {
    conversation: String,
    text: String,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    message_id: Option<String>,
}

async fn google_chat_inbound(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GoogleChatInboundRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;

    let session_key = if let Some(thread_key) = req.thread_key.as_deref().filter(|s| !s.is_empty()) {
        format!("google_chat:{}:{}", req.space, thread_key)
    } else {
        format!("google_chat:{}", req.space)
    };

    if let Some(message_id) = req.message_id.as_deref() {
        let key = format!("google_chat:{}", message_id);
        if dedupe_seen(&state, &key, 600).await {
            return Ok(Json(json!({
                "status": "duplicate_ignored",
                "session_key": session_key
            })));
        }
    }

    let mut inbound = Message::new("google_chat", &session_key, Role::User, &req.text)
        .with_sender(req.sender.as_deref().unwrap_or("google_chat_user"));
    inbound
        .metadata
        .insert("google_chat_space".to_string(), req.space.clone());
    if let Some(message_id) = req.message_id {
        inbound
            .metadata
            .insert("google_chat_message_id".to_string(), message_id);
    }

    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "status": "accepted", "session_key": session_key })))
}

async fn zalo_inbound(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<ZaloInboundRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;

    let session_key = format!("zalo:{}", req.from);
    if let Some(message_id) = req.message_id.as_deref() {
        let key = format!("zalo:{}", message_id);
        if dedupe_seen(&state, &key, 600).await {
            return Ok(Json(json!({
                "status": "duplicate_ignored",
                "session_key": session_key
            })));
        }
    }

    let mut inbound = Message::new("zalo", &session_key, Role::User, &req.text).with_sender(&req.from);
    if let Some(sender_name) = req.sender_name {
        inbound
            .metadata
            .insert("sender_name".to_string(), sender_name);
    }
    if let Some(message_id) = req.message_id {
        inbound
            .metadata
            .insert("zalo_message_id".to_string(), message_id);
    }

    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "status": "accepted", "session_key": session_key })))
}

async fn teams_inbound(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TeamsInboundRequest>,
) -> Result<Json<Value>, StatusCode> {
    check_auth(&state, &headers)?;

    let session_key = format!("teams:{}", req.conversation);
    if let Some(message_id) = req.message_id.as_deref() {
        let key = format!("teams:{}", message_id);
        if dedupe_seen(&state, &key, 600).await {
            return Ok(Json(json!({
                "status": "duplicate_ignored",
                "session_key": session_key
            })));
        }
    }

    let mut inbound = Message::new("teams", &session_key, Role::User, &req.text)
        .with_sender(req.sender.as_deref().unwrap_or("teams_user"));
    if let Some(message_id) = req.message_id {
        inbound
            .metadata
            .insert("teams_message_id".to_string(), message_id);
    }

    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "status": "accepted", "session_key": session_key })))
}

async fn slack_inbound(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, StatusCode> {
    if let Some(secret) = state.slack_signing_secret.as_deref() {
        if !secret.is_empty() && !verify_slack_signature(&headers, &body, secret) {
            record_channel_error(&state, "slack", "signature_invalid").await;
            return Err(StatusCode::UNAUTHORIZED);
        }
    } else {
        check_auth(&state, &headers)?;
    }

    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            record_channel_error(&state, "slack", "invalid_json").await;
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    if parsed
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == "url_verification")
    {
        let challenge = parsed
            .get("challenge")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        return Ok(Json(json!({ "challenge": challenge })));
    }

    // Full Slack Events API envelope
    if parsed
        .get("type")
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == "event_callback")
    {
        let event = parsed
            .get("event")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let event_type = event
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if event_type != "message" {
            return Ok(Json(json!({ "status": "ignored_non_message_event" })));
        }

        // Ignore bot-generated updates
        if event.get("bot_id").is_some()
            || event
                .get("subtype")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s == "bot_message" || s == "message_changed")
        {
            return Ok(Json(json!({ "status": "ignored_bot_event" })));
        }

        let channel = event
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let text = event
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if channel.is_empty() || text.is_empty() {
            return Ok(Json(json!({ "status": "ignored_incomplete_message" })));
        }

        let thread_ts = event
            .get("thread_ts")
            .and_then(|v| v.as_str())
            .or_else(|| event.get("ts").and_then(|v| v.as_str()));
        let session_key = if let Some(ts) = thread_ts {
            format!("slack:{}:{}", channel, ts)
        } else {
            format!("slack:{}", channel)
        };

        // Deduplicate by envelope event_id or message keys
        if let Some(event_id) = parsed.get("event_id").and_then(|v| v.as_str()) {
            if dedupe_seen(&state, &format!("slack:event:{}", event_id), 600).await {
                return Ok(Json(json!({ "status": "duplicate_ignored", "session_key": session_key })));
            }
        }
        if let Some(client_msg_id) = event.get("client_msg_id").and_then(|v| v.as_str()) {
            if dedupe_seen(&state, &format!("slack:msg:{}", client_msg_id), 600).await {
                return Ok(Json(json!({ "status": "duplicate_ignored", "session_key": session_key })));
            }
        } else if let Some(ts) = event.get("ts").and_then(|v| v.as_str()) {
            let fallback_key = format!("slack:{}:{}", channel, ts);
            if dedupe_seen(&state, &fallback_key, 600).await {
                return Ok(Json(json!({ "status": "duplicate_ignored", "session_key": session_key })));
            }
        }

        let mut inbound = Message::new("slack", &session_key, Role::User, text)
            .with_sender(event.get("user").and_then(|v| v.as_str()).unwrap_or("slack_user"));
        inbound
            .metadata
            .insert("slack_channel".to_string(), channel.to_string());
        if let Some(ts) = event.get("ts").and_then(|v| v.as_str()) {
            inbound
                .metadata
                .insert("slack_ts".to_string(), ts.to_string());
        }
        if let Some(event_id) = parsed.get("event_id").and_then(|v| v.as_str()) {
            inbound
                .metadata
                .insert("slack_event_id".to_string(), event_id.to_string());
        }

        state
            .bus
            .publish(Event::InboundMessage(inbound))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        return Ok(Json(json!({ "status": "accepted", "session_key": session_key })));
    }

    // Fallback simple JSON body (manual integration path)
    let req: SlackInboundRequest =
        serde_json::from_value(parsed.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;

    let session_key = if let Some(thread_ts) = req.thread_ts.as_deref() {
        format!("slack:{}:{}", req.channel, thread_ts)
    } else {
        format!("slack:{}", req.channel)
    };

    let fallback_dedupe = format!(
        "slack:fallback:{}:{}:{}",
        req.channel,
        req.thread_ts.clone().unwrap_or_default(),
        req.text
    );
    if dedupe_seen(&state, &fallback_dedupe, 120).await {
        return Ok(Json(json!({ "status": "duplicate_ignored", "session_key": session_key })));
    }

    let mut inbound = Message::new("slack", &session_key, Role::User, &req.text)
        .with_sender(req.user.as_deref().unwrap_or("slack_user"));
    inbound
        .metadata
        .insert("slack_channel".to_string(), req.channel.clone());

    state
        .bus
        .publish(Event::InboundMessage(inbound))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "status": "accepted", "session_key": session_key })))
}

fn verify_slack_signature(headers: &HeaderMap, body: &[u8], signing_secret: &str) -> bool {
    let ts = headers
        .get("x-slack-request-timestamp")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_default();
    let signature = headers
        .get("x-slack-signature")
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default();

    if ts == 0 || signature.is_empty() {
        return false;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    if (now - ts).abs() > 300 {
        return false;
    }

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = match HmacSha256::new_from_slice(signing_secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let basestring = format!("v0:{}:{}", ts, String::from_utf8_lossy(body));
    mac.update(basestring.as_bytes());
    let digest = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    signature == format!("v0={}", digest)
}

async fn handle_ws_events(
    mut socket: WebSocket,
    bus: Arc<MessageBus>,
    metrics: Arc<MetricsStore>,
    ws_heartbeat_secs: u64,
) {
    let mut rx = bus.subscribe();
    let mut ticker = interval(Duration::from_secs(ws_heartbeat_secs));

    let connected = json!({
        "type": "connected",
        "message": "event stream ready",
        "metrics": metrics.snapshot(),
    });

    if socket
        .send(WsMessage::Text(connected.to_string().into()))
        .await
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let heartbeat = json!({
                    "type": "heartbeat",
                    "metrics": metrics.snapshot(),
                });
                if socket.send(WsMessage::Text(heartbeat.to_string().into())).await.is_err() {
                    break;
                }
            }
            event = rx.recv() => {
                match event {
                    Ok(Event::InboundMessage(message)) => {
                        let payload = json!({
                            "type": "inbound_message",
                            "message": message,
                        });
                        if socket.send(WsMessage::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Event::OutboundMessage(message)) => {
                        let payload = json!({
                            "type": "outbound_message",
                            "message": message,
                        });
                        if socket.send(WsMessage::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(Event::SystemLog { level, message }) => {
                        let payload = json!({
                            "type": "system_log",
                            "level": level,
                            "message": message,
                        });
                        if socket.send(WsMessage::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        let payload = json!({
                            "type": "lagged",
                            "skipped": skipped,
                        });
                        if socket.send(WsMessage::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }
    }
}
