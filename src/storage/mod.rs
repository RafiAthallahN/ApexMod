use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::config::CalibrationProfile;
use crate::physics::ModulationEvent;

#[derive(Clone)]
pub struct TelemetryStore {
    conn: Arc<Mutex<Connection>>,
}

impl TelemetryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable Write-Ahead Logging for concurrent non-blocking telemetry writes (Schema.md §2.5)
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS calibration_profiles (
                id TEXT PRIMARY KEY,
                device_name TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS session_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_ms INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                velocity REAL NOT NULL,
                score REAL NOT NULL
            );",
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
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

    pub fn record_event(&self, timestamp_ms: u64, event: ModulationEvent, velocity: f32, score: f32) {
        let event_str = match event {
            ModulationEvent::BrakeDump => "BRAKE_DUMP",
            ModulationEvent::ThrottleSnap => "THROTTLE_SNAP",
        };
        if let Ok(guard) = self.conn.lock() {
            let _ = guard.execute(
                "INSERT INTO session_events (timestamp_ms, event_type, velocity, score)
                 VALUES (?1, ?2, ?3, ?4)",
                params![timestamp_ms as i64, event_str, velocity as f64, score as f64],
            );
        }
    }
}
