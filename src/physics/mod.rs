use serde::{Deserialize, Serialize};
use crate::config::CalibrationProfile;
use crate::input::acc_udp::AccCarTelemetry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModulationEvent {
    #[serde(rename = "BRAKE_DUMP")]
    BrakeDump,
    #[serde(rename = "THROTTLE_SNAP")]
    ThrottleSnap,
}

/// Time-synced Telemetry Frame merging evdev triggers + ACC UDP car state (PRD v2.0.0 §2.2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryFrame {
    /// Master frame timestamp in milliseconds
    pub t: u64,
    /// Normalized & EMA-filtered brake position [0.0, 1.0]
    pub brk: f32,
    /// Normalized & EMA-filtered throttle position [0.0, 1.0]
    pub thr: f32,
    /// Instantaneous brake velocity d(brk)/dt (1/s)
    pub d_brk: f32,
    /// Instantaneous throttle velocity d(thr)/dt (1/s)
    pub d_thr: f32,
    /// Instantaneous brake jerk d^2(brk)/dt^2
    pub j_brk: f32,
    /// Instantaneous throttle jerk d^2(thr)/dt^2
    pub j_thr: f32,
    /// Triggered acoustic/telemetry fault event, if any
    pub event: Option<ModulationEvent>,
    /// Continuous 0-100% Modulation Index smoothness score
    pub score: f32,
    /// Whether a hardware or simulated controller is actively streaming
    pub connected: bool,
    /// Controller identifier
    pub device_name: String,
    /// ACC UDP: Vehicle speed (km/h)
    pub speed_kmh: f32,
    /// ACC UDP: Sequential gear (-1 = R, 0 = N, 1..6 = 1st..6th)
    pub gear: i32,
    /// ACC UDP: ABS intervention active flag
    pub abs_active: bool,
    /// ACC UDP: Traction Control slip cut active flag
    pub tc_active: bool,
    /// ACC UDP: Current lap number
    pub lap_index: i32,
    /// ACC UDP: Current lap elapsed time in ms
    pub lap_time_ms: u32,
    /// ACC UDP: Active circuit identifier
    pub track_id: String,
    /// ACC UDP: Active vehicle model identifier
    pub car_model: String,
    /// ACC UDP listener connection state
    pub udp_connected: bool,
}

pub struct ModulationEvaluator {
    prev_brk: f32,
    prev_thr: f32,
    prev_d_brk: f32,
    prev_d_thr: f32,
    /// Tracks recent high-brake zone (> 0.70) to evaluate trail-braking release dumps (FR-2.2)
    recent_high_brake_hold: f32,
    /// Tracks recent low-throttle zone (< 0.20) to evaluate corner-exit throttle snaps (FR-2.3)
    recent_low_throttle_hold: f32,
    /// Cooldown timers (seconds) to avoid re-triggering identical fault on every 4ms tick
    dump_cooldown: f32,
    snap_cooldown: f32,
    /// Rolling Modulation Index [0.0, 100.0]
    rolling_score: f32,
}

impl Default for ModulationEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ModulationEvaluator {
    pub fn new() -> Self {
        Self {
            prev_brk: 0.0,
            prev_thr: 0.0,
            prev_d_brk: 0.0,
            prev_d_thr: 0.0,
            recent_high_brake_hold: 0.0,
            recent_low_throttle_hold: 0.0,
            dump_cooldown: 0.0,
            snap_cooldown: 0.0,
            rolling_score: 100.0,
        }
    }

    /// Step the physics evaluator forward by `dt` seconds (target: 0.004s at 250Hz)
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        raw_brk: f32,
        raw_thr: f32,
        dt: f32,
        profile: &CalibrationProfile,
        timestamp_ms: u64,
        connected: bool,
        car: &AccCarTelemetry,
    ) -> TelemetryFrame {
        let dt_safe = dt.max(0.0005);

        // 1. Deadzone normalization (FR-1.3)
        let norm_brk = CalibrationProfile::normalize_axis(raw_brk, &profile.brake_deadzone);
        let norm_thr = CalibrationProfile::normalize_axis(raw_thr, &profile.throttle_deadzone);

        // 2. First-order EMA smoothing (Schema.md §2.1)
        let alpha = profile.ema_alpha.clamp(0.05, 1.0);
        let brk = alpha * norm_brk + (1.0 - alpha) * self.prev_brk;
        let thr = alpha * norm_thr + (1.0 - alpha) * self.prev_thr;

        // 3. First derivative: instantaneous velocity (FR-2.1)
        let d_brk = (brk - self.prev_brk) / dt_safe;
        let d_thr = (thr - self.prev_thr) / dt_safe;

        // 4. Second derivative: instantaneous jerk (FR-2.4)
        let j_brk = (d_brk - self.prev_d_brk) / dt_safe;
        let j_thr = (d_thr - self.prev_d_thr) / dt_safe;

        // Update envelope memory windows (approx 350ms window for transition context)
        if brk > 0.70 {
            self.recent_high_brake_hold = 0.35;
        } else {
            self.recent_high_brake_hold = (self.recent_high_brake_hold - dt_safe).max(0.0);
        }

        if thr < 0.20 {
            self.recent_low_throttle_hold = 0.35;
        } else {
            self.recent_low_throttle_hold = (self.recent_low_throttle_hold - dt_safe).max(0.0);
        }

        self.dump_cooldown = (self.dump_cooldown - dt_safe).max(0.0);
        self.snap_cooldown = (self.snap_cooldown - dt_safe).max(0.0);

        // 5. Event detection (FR-2.2 & FR-2.3)
        let mut event = None;

        // FR-2.2: Brake Dump when dropping from > 0.70 toward 0.0 faster than -V_dump_threshold
        if self.recent_high_brake_hold > 0.0
            && d_brk < -profile.dump_velocity_threshold
            && self.dump_cooldown <= 0.0
        {
            event = Some(ModulationEvent::BrakeDump);
            self.dump_cooldown = 0.40;
            self.rolling_score = (self.rolling_score - 6.5).max(0.0);
        }

        // FR-2.3: Throttle Snap when rising from < 0.20 toward 1.0 faster than +V_snap_threshold
        if event.is_none()
            && self.recent_low_throttle_hold > 0.0
            && thr > 0.25
            && d_thr > profile.snap_velocity_threshold
            && self.snap_cooldown <= 0.0
        {
            event = Some(ModulationEvent::ThrottleSnap);
            self.snap_cooldown = 0.40;
            self.rolling_score = (self.rolling_score - 5.5).max(0.0);
        }

        // 6. Continuous Modulation Index score (0-100%) based on jerk during active transitions (FR-2.4)
        let active_transition = d_brk.abs() > 0.08 || d_thr.abs() > 0.08;
        if active_transition {
            let combined_jerk = (j_brk.abs() * 0.6 + j_thr.abs() * 0.4) / 60.0;
            let instantaneous_quality = (100.0 - combined_jerk.min(100.0)).clamp(0.0, 100.0);
            let blend = (dt_safe * 2.5).min(1.0);
            self.rolling_score = (self.rolling_score * (1.0 - blend)
                + instantaneous_quality * blend)
                .clamp(0.0, 100.0);
        }

        self.prev_brk = brk;
        self.prev_thr = thr;
        self.prev_d_brk = d_brk;
        self.prev_d_thr = d_thr;

        TelemetryFrame {
            t: timestamp_ms,
            brk: (brk * 1000.0).round() / 1000.0,
            thr: (thr * 1000.0).round() / 1000.0,
            d_brk: (d_brk * 1000.0).round() / 1000.0,
            d_thr: (d_thr * 1000.0).round() / 1000.0,
            j_brk: (j_brk * 100.0).round() / 100.0,
            j_thr: (j_thr * 100.0).round() / 100.0,
            event,
            score: (self.rolling_score * 10.0).round() / 10.0,
            connected,
            device_name: profile.device_name.clone(),
            speed_kmh: car.speed_kmh,
            gear: car.gear,
            abs_active: car.abs_active,
            tc_active: car.tc_active,
            lap_index: car.lap_index,
            lap_time_ms: car.lap_time_ms,
            track_id: car.track_id.clone(),
            car_model: car.car_model.clone(),
            udp_connected: car.udp_connected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_brake_dump_and_throttle_snap() {
        let mut eval = ModulationEvaluator::new();
        let profile = CalibrationProfile {
            ema_alpha: 1.0, // Unfiltered for deterministic unit test
            ..Default::default()
        };
        let car = AccCarTelemetry::default();

        // Hold heavy brake (> 0.70)
        let f1 = eval.step(0.95, 0.0, 0.004, &profile, 1000, true, &car);
        assert_eq!(f1.event, None);

        // Abruptly dump brake to 0.10 in 4ms -> d_brk = -212.5 / s
        let f2 = eval.step(0.10, 0.0, 0.004, &profile, 1004, true, &car);
        assert_eq!(f2.event, Some(ModulationEvent::BrakeDump));

        // Abruptly snap throttle from 0.0 to 0.90 in 4ms -> d_thr = +225.0 / s
        let f3 = eval.step(0.0, 0.90, 0.004, &profile, 1008, true, &car);
        assert_eq!(f3.event, Some(ModulationEvent::ThrottleSnap));
    }
}
