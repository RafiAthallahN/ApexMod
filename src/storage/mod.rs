use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::CalibrationProfile;
use crate::physics::{ModulationEvent, TelemetryFrame};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub track_id: String,
    pub car_model: String,
    pub controller_name: String,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub total_laps: i32,
    pub avg_modulation_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapSummary {
    pub id: String,
    pub session_id: String,
    pub lap_number: i32,
    pub lap_time_ms: Option<i64>,
    pub is_valid: bool,
    pub abs_events_count: i32,
    pub tc_events_count: i32,
    pub brake_dump_count: i32,
    pub throttle_snap_count: i32,
    pub modulation_score: f32,
}

#[derive(Clone)]
pub struct TelemetryStore {
    conn: Arc<Mutex<Connection>>,
    frame_tx: SyncSender<TelemetryFrame>,
}

impl TelemetryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path: PathBuf = path.as_ref().to_path_buf();
        let conn = Connection::open(&db_path)?;

        // Enable Write-Ahead Logging for high-throughput concurrent logging (PRD v2.0.0 §3)
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS calibration_profiles (
                id TEXT PRIMARY KEY,
                device_name TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                track_id TEXT NOT NULL,
                car_model TEXT NOT NULL,
                controller_name TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                total_laps INTEGER DEFAULT 0,
                avg_modulation_score REAL DEFAULT 0.0
            );

            CREATE TABLE IF NOT EXISTS laps (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                lap_number INTEGER NOT NULL,
                lap_time_ms INTEGER,
                is_valid BOOLEAN DEFAULT 1,
                abs_events_count INTEGER DEFAULT 0,
                tc_events_count INTEGER DEFAULT 0,
                brake_dump_count INTEGER DEFAULT 0,
                throttle_snap_count INTEGER DEFAULT 0,
                modulation_score REAL DEFAULT 0.0,
                FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS telemetry_frames (
                lap_id TEXT NOT NULL,
                timestamp_ms INTEGER NOT NULL,
                trigger_brake REAL NOT NULL,
                trigger_throttle REAL NOT NULL,
                d_brake_dt REAL NOT NULL,
                d_throttle_dt REAL NOT NULL,
                speed_kmh REAL NOT NULL,
                gear INTEGER NOT NULL,
                abs_active INTEGER NOT NULL,
                tc_active INTEGER NOT NULL,
                PRIMARY KEY (lap_id, timestamp_ms),
                FOREIGN KEY(lap_id) REFERENCES laps(id) ON DELETE CASCADE
            ) WITHOUT ROWID;

            CREATE INDEX IF NOT EXISTS idx_telemetry_lap ON telemetry_frames(lap_id, timestamp_ms);",
        )?;

        // Bounded ring buffer queue (maxsize = 10_000) for non-blocking 60Hz/120Hz/250Hz ingestion (PRD §4)
        let (frame_tx, frame_rx) = mpsc::sync_channel::<TelemetryFrame>(10_000);
        let writer_db_path = db_path.clone();

        std::thread::Builder::new()
            .name("apexmod-db-writer".to_string())
            .spawn(move || {
                run_batch_writer_loop(writer_db_path, frame_rx);
            })
            .expect("Failed to spawn async SQLite batch writer thread");

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            frame_tx,
        })
    }

    /// Called from the real-time polling thread. Never blocks disk I/O (PRD v2.0.0 §4).
    pub fn push_frame(&self, frame: TelemetryFrame) {
        match self.frame_tx.try_send(frame) {
            Ok(_) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }

    pub fn load_or_default_profile(&self, id: &str) -> CalibrationProfile {
        if let Ok(guard) = self.conn.lock() {
            if let Ok(json_str) = guard.query_row(
                "SELECT payload_json FROM calibration_profiles WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            ) {
                if let Ok(profile) = serde_json::from_str::<CalibrationProfile>(&json_str) {
                    return profile;
                }
            }
        }
        let default_profile = CalibrationProfile::default();
        let _ = self.save_profile(&default_profile);
        default_profile
    }

    pub fn save_profile(&self, profile: &CalibrationProfile) -> Result<()> {
        if let Ok(guard) = self.conn.lock() {
            let json_str = serde_json::to_string(profile).unwrap_or_default();
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            guard.execute(
                "INSERT INTO calibration_profiles (id, device_name, payload_json, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   device_name = excluded.device_name,
                   payload_json = excluded.payload_json,
                   updated_at = excluded.updated_at",
                params![profile.id, profile.device_name, json_str, now_ms],
            )?;
        }
        Ok(())
    }

    pub fn list_recent_sessions(&self, limit: usize) -> Vec<SessionSummary> {
        let mut out = Vec::new();
        if let Ok(guard) = self.conn.lock() {
            if let Ok(mut stmt) = guard.prepare(
                "SELECT id, track_id, car_model, controller_name, started_at, ended_at, total_laps, avg_modulation_score
                 FROM sessions
                 ORDER BY started_at DESC
                 LIMIT ?1",
            ) {
                if let Ok(rows) = stmt.query_map(params![limit as i64], |r| {
                    Ok(SessionSummary {
                        id: r.get(0)?,
                        track_id: r.get(1)?,
                        car_model: r.get(2)?,
                        controller_name: r.get(3)?,
                        started_at: r.get(4)?,
                        ended_at: r.get(5)?,
                        total_laps: r.get(6)?,
                        avg_modulation_score: r.get(7)?,
                    })
                }) {
                    for s in rows.flatten() {
                        out.push(s);
                    }
                }
            }
        }
        out
    }
}

fn generate_uuid_v4_like(prefix: &str, seed_ms: u64, counter: u64) -> String {
    format!("{}-{:012x}-{:06x}", prefix, seed_ms, counter & 0xFFFFFF)
}

fn run_batch_writer_loop(db_path: PathBuf, rx: mpsc::Receiver<TelemetryFrame>) {
    let Ok(mut conn) = Connection::open(&db_path) else {
        return;
    };
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");

    let start_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut id_counter = 1u64;
    let session_id = generate_uuid_v4_like("sess", start_ms, id_counter);
    let mut current_lap_num: i32 = 1;
    id_counter += 1;
    let mut current_lap_id = generate_uuid_v4_like("lap", start_ms, id_counter);

    let _ = conn.execute(
        "INSERT OR IGNORE INTO sessions (id, track_id, car_model, controller_name, started_at, total_laps, avg_modulation_score)
         VALUES (?1, 'monza', 'porsche_992_gt3_r', 'Generic Controller', ?2, 1, 100.0)",
        params![session_id, start_ms as i64],
    );

    let _ = conn.execute(
        "INSERT OR IGNORE INTO laps (id, session_id, lap_number, is_valid, modulation_score)
         VALUES (?1, ?2, ?3, 1, 100.0)",
        params![current_lap_id, session_id, current_lap_num],
    );

    let mut batch: Vec<(String, TelemetryFrame)> = Vec::with_capacity(320);
    let mut last_flush = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(frame) => {
                if frame.connected {
                    if frame.lap_index != current_lap_num && frame.lap_index > 0 {
                        current_lap_num = frame.lap_index;
                        id_counter += 1;
                        current_lap_id = generate_uuid_v4_like("lap", frame.t, id_counter);
                        let _ = conn.execute(
                            "INSERT OR IGNORE INTO laps (id, session_id, lap_number, is_valid, modulation_score)
                             VALUES (?1, ?2, ?3, 1, ?4)",
                            params![current_lap_id, session_id, current_lap_num, frame.score],
                        );
                    }
                    batch.push((current_lap_id.clone(), frame));
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }

        // Flush every 500ms or when batch size hits 300 samples (PRD v2.0.0 §4)
        if !batch.is_empty()
            && (batch.len() >= 300 || last_flush.elapsed() >= Duration::from_millis(500))
        {
            if let Ok(tx) = conn.transaction() {
                for (lap_id, f) in &batch {
                    let _ = tx.execute(
                        "INSERT OR REPLACE INTO telemetry_frames (
                            lap_id, timestamp_ms, trigger_brake, trigger_throttle,
                            d_brake_dt, d_throttle_dt, speed_kmh, gear, abs_active, tc_active
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            lap_id,
                            f.t as i64,
                            f.brk as f64,
                            f.thr as f64,
                            f.d_brk as f64,
                            f.d_thr as f64,
                            f.speed_kmh as f64,
                            f.gear,
                            i32::from(f.abs_active),
                            i32::from(f.tc_active),
                        ],
                    );

                    let dump_inc = i32::from(f.event == Some(ModulationEvent::BrakeDump));
                    let snap_inc = i32::from(f.event == Some(ModulationEvent::ThrottleSnap));
                    let abs_inc = i32::from(f.abs_active);
                    let tc_inc = i32::from(f.tc_active);

                    let _ = tx.execute(
                        "UPDATE laps SET
                            lap_time_ms = ?1,
                            abs_events_count = abs_events_count + ?2,
                            tc_events_count = tc_events_count + ?3,
                            brake_dump_count = brake_dump_count + ?4,
                            throttle_snap_count = throttle_snap_count + ?5,
                            modulation_score = ?6
                         WHERE id = ?7",
                        params![
                            f.lap_time_ms as i64,
                            abs_inc,
                            tc_inc,
                            dump_inc,
                            snap_inc,
                            f.score as f64,
                            lap_id
                        ],
                    );
                }

                if let Some((_, last_f)) = batch.last() {
                    let _ = tx.execute(
                        "UPDATE sessions SET
                            track_id = ?1,
                            car_model = ?2,
                            controller_name = ?3,
                            ended_at = ?4,
                            total_laps = MAX(total_laps, ?5),
                            avg_modulation_score = ?6
                         WHERE id = ?7",
                        params![
                            last_f.track_id,
                            last_f.car_model,
                            last_f.device_name,
                            last_f.t as i64,
                            last_f.lap_index,
                            last_f.score as f64,
                            session_id
                        ],
                    );
                }

                let _ = tx.commit();
            }
            batch.clear();
            last_flush = Instant::now();
        }
    }
}
