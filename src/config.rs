use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadzoneConfig {
    pub inner: f32,
    pub outer: f32,
}

impl Default for DeadzoneConfig {
    fn default() -> Self {
        Self {
            inner: 0.02,
            outer: 0.98,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub id: String,
    pub device_name: String,
    pub brake_deadzone: DeadzoneConfig,
    pub throttle_deadzone: DeadzoneConfig,
    /// EMA smoothing factor alpha in (0.0, 1.0]. Lower = smoother, Higher = more responsive.
    pub ema_alpha: f32,
    /// Brake dump threshold in normalized units per second (e.g., 1.50 = 150%/s release rate).
    pub dump_velocity_threshold: f32,
    /// Throttle snap threshold in normalized units per second (e.g., 1.80 = 180%/s ramp rate).
    pub snap_velocity_threshold: f32,
    /// Audio mute state
    pub audio_muted: bool,
    /// Master audio volume [0.0, 1.0]
    pub audio_volume: f32,
}

impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            id: "default".to_string(),
            device_name: "Generic Controller".to_string(),
            brake_deadzone: DeadzoneConfig::default(),
            throttle_deadzone: DeadzoneConfig::default(),
            ema_alpha: 0.35,
            dump_velocity_threshold: 1.50,
            snap_velocity_threshold: 1.80,
            audio_muted: false,
            audio_volume: 0.25,
        }
    }
}

impl CalibrationProfile {
    /// Normalize a raw [0.0, 1.0] value through inner/outer deadzones into [0.0, 1.0].
    pub fn normalize_axis(raw: f32, dz: &DeadzoneConfig) -> f32 {
        let clamped = raw.clamp(0.0, 1.0);
        if clamped <= dz.inner {
            0.0
        } else if clamped >= dz.outer {
            1.0
        } else {
            let span = (dz.outer - dz.inner).max(f32::EPSILON);
            ((clamped - dz.inner) / span).clamp(0.0, 1.0)
        }
    }
}
