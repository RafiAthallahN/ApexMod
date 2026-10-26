pub mod audio;
pub mod config;
pub mod input;
pub mod physics;
pub mod server;
pub mod storage;

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

use audio::EarconEngine;
use input::InputManager;
use physics::{ModulationEvaluator, ModulationEvent};
use server::{run_dedicated_ws_server, run_http_dashboard_server, SharedAppState};
use storage::TelemetryStore;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "apex_mod=info".into()),
        )
        .init();

    tracing::info!("Starting ApexMod v2.0 Core Daemon (250Hz Polling / ACC UDP :9000 / 60Hz WS)");

    // Initialize local SQLite store in WAL mode with async ring-buffer batch writer (PRD v2.0.0 §3 & §4)
    let store = TelemetryStore::open("apexmod.db").expect("Failed to open apexmod.db SQLite store");
    let initial_profile = store.load_or_default_profile("default");

    let profile = Arc::new(RwLock::new(initial_profile));
    let sim_default = std::env::args().any(|a| a == "--sim" || a == "--simulate")
        || std::env::var("APEXMOD_SIM").is_ok_and(|v| v == "1" || v == "true");
    let simulation_enabled = Arc::new(RwLock::new(sim_default));

    let (tx_frames, _rx) = broadcast::channel::<String>(128);

    // Spawn low-latency stereo procedural audio engine (FR-3.1..3.4)
    let earcon_engine = EarconEngine::spawn();

    // Spawn 250Hz deterministic input & physics thread (FR-1.4, PRD v2.0.0 §1)
    let loop_profile = Arc::clone(&profile);
    let loop_sim = Arc::clone(&simulation_enabled);
    let loop_tx = tx_frames.clone();
    let loop_store = store.clone();

    std::thread::Builder::new()
        .name("apexmod-250hz-core".to_string())
        .spawn(move || {
            let mut input_mgr = InputManager::new(sim_default);
            let mut evaluator = ModulationEvaluator::new();

            let tick_interval = Duration::from_micros(4_000); // 250 Hz (4.0 ms)
            let ws_interval = Duration::from_micros(16_667);  // 60 Hz (~16.67 ms)
            let mut last_tick = Instant::now();
            let mut last_ws_broadcast = Instant::now();
            let mut sticky_event: Option<ModulationEvent> = None;

            loop {
                let now = Instant::now();
                let dt = now.duration_since(last_tick).as_secs_f32().max(0.0005);
                last_tick = now;

                if let Ok(sim_flag) = loop_sim.read() {
                    input_mgr.simulation_enabled = *sim_flag;
                }

                let (raw_brk, raw_thr, connected, device_name, car_state) = input_mgr.poll(dt);

                let mut active_profile = loop_profile
                    .read()
                    .map(|p| p.clone())
                    .unwrap_or_default();
                active_profile.device_name = device_name;

                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let mut frame = evaluator.step(
                    raw_brk,
                    raw_thr,
                    dt,
                    &active_profile,
                    timestamp_ms,
                    connected,
                    &car_state,
                );

                // Drive low-latency stereo earcons at 250Hz
                earcon_engine.update(
                    &frame,
                    active_profile.audio_muted,
                    active_profile.audio_volume,
                );

                if let Some(ev) = frame.event {
                    sticky_event = Some(ev);
                }

                // Broadcast over WebSocket and enqueue to Async Storage Ring Buffer at 60Hz (PRD v2.0.0 §1 & §4)
                if now.duration_since(last_ws_broadcast) >= ws_interval {
                    last_ws_broadcast = now;
                    if frame.event.is_none() && sticky_event.is_some() {
                        frame.event = sticky_event.take();
                    } else {
                        sticky_event = None;
                    }

                    // Non-blocking enqueue to SQLite WAL ring buffer
                    loop_store.push_frame(frame.clone());

                    if let Ok(json_payload) = serde_json::to_string(&frame) {
                        let _ = loop_tx.send(json_payload);
                    }
                }

                let elapsed = now.elapsed();
                if elapsed < tick_interval {
                    std::thread::sleep(tick_interval - elapsed);
                }
            }
        })
        .expect("Failed to spawn 250Hz core polling thread");

    let ws_addr: SocketAddr = "0.0.0.0:8765".parse().unwrap();
    let http_addr: SocketAddr = "0.0.0.0:3000".parse().unwrap();

    let app_state = SharedAppState {
        profile,
        simulation_enabled,
        tx_frames: tx_frames.clone(),
        store,
    };

    tokio::join!(
        run_dedicated_ws_server(ws_addr, tx_frames),
        run_http_dashboard_server(http_addr, app_state),
    );
}
