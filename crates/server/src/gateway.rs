use axum::{extract::State, routing::get, Json, Router};
use pocketclaw_core::bus::MessageBus;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    bus: Arc<MessageBus>,
}

pub struct Gateway {
    bus: Arc<MessageBus>,
    port: u16,
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

impl Gateway {
    pub fn new(bus: Arc<MessageBus>, port: u16) -> Self {
        Self { bus, port }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let state = AppState {
            bus: self.bus.clone(),
        };

        let app = Router::new()
            .route("/health", get(health_check))
            .route("/api/status", get(api_status))
            .with_state(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        info!("Gateway listening on {}", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
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
