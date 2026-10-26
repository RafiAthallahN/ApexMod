use axum::{
    extract::{
        ws::{Message as AxumWsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;
use tower_http::{cors::CorsLayer, services::ServeDir};

use crate::config::CalibrationProfile;
use crate::storage::{SessionSummary, TelemetryStore};

#[derive(Clone)]
pub struct SharedAppState {
    pub profile: Arc<RwLock<CalibrationProfile>>,
    pub simulation_enabled: Arc<RwLock<bool>>,
    pub tx_frames: broadcast::Sender<String>,
    pub store: TelemetryStore,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SimulationTogglePayload {
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub profile: CalibrationProfile,
    pub simulation_enabled: bool,
}

/// Spawns the dedicated 60Hz WebSocket broadcast server at 0.0.0.0:8765 (FR-4.1 / Schema.md §2.3)
pub async fn run_dedicated_ws_server(addr: SocketAddr, tx_frames: broadcast::Sender<String>) {
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind dedicated WebSocket server on {}: {}", addr, e);
            return;
        }
    };
    tracing::info!("WebSocket telemetry server listening on ws://{}", addr);

    while let Ok((stream, peer)) = listener.accept().await {
        let mut rx = tx_frames.subscribe();
        tokio::spawn(async move {
            let Ok(ws_stream) = tokio_tungstenite::accept_async(stream).await else {
                return;
            };
            tracing::debug!("Dedicated WS client connected: {}", peer);
            let (mut write, mut read) = ws_stream.split();

            loop {
                tokio::select! {
                    Ok(payload) = rx.recv() => {
                        if write.send(TungsteniteMessage::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(TungsteniteMessage::Close(_))) | None | Some(Err(_)) => break,
                            _ => {}
                        }
                    }
                }
            }
        });
    }
}

/// Spawns the built-in Axum HTTP server for the Zero-Install Dashboard & Control API (FR-4.2)
pub async fn run_http_dashboard_server(addr: SocketAddr, state: SharedAppState) {
    let app = Router::new()
        .route("/api/status", get(get_status))
        .route("/api/sessions", get(get_sessions))
        .route("/api/config", post(update_config))
        .route("/api/simulate", post(toggle_simulation))
        .route("/ws", get(axum_ws_handler))
        .fallback_service(ServeDir::new("web/dist"))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind HTTP dashboard server on {}: {}", addr, e);
            return;
        }
    };
    tracing::info!("Built-in Web Dashboard server listening on http://{}", addr);
    let _ = axum::serve(listener, app).await;
}

async fn get_status(State(state): State<SharedAppState>) -> Json<StatusResponse> {
    let profile = state
        .profile
        .read()
        .map(|p| p.clone())
        .unwrap_or_default();
    let simulation_enabled = state
        .simulation_enabled
        .read()
        .map(|s| *s)
        .unwrap_or(false);
    Json(StatusResponse {
        profile,
        simulation_enabled,
    })
}

async fn get_sessions(State(state): State<SharedAppState>) -> Json<Vec<SessionSummary>> {
    Json(state.store.list_recent_sessions(20))
}

async fn update_config(
    State(state): State<SharedAppState>,
    Json(new_profile): Json<CalibrationProfile>,
) -> Json<CalibrationProfile> {
    if let Ok(mut guard) = state.profile.write() {
        *guard = new_profile.clone();
    }
    let _ = state.store.save_profile(&new_profile);
    Json(new_profile)
}

async fn toggle_simulation(
    State(state): State<SharedAppState>,
    Json(payload): Json<SimulationTogglePayload>,
) -> Json<SimulationTogglePayload> {
    if let Ok(mut guard) = state.simulation_enabled.write() {
        *guard = payload.enabled;
    }
    Json(payload)
}

async fn axum_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedAppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_axum_ws(socket, state))
}

async fn handle_axum_ws(mut socket: WebSocket, state: SharedAppState) {
    let mut rx = state.tx_frames.subscribe();
    while let Ok(payload) = rx.recv().await {
        if socket
            .send(AxumWsMessage::Text(payload.into()))
            .await
            .is_err()
        {
            break;
        }
    }
}
