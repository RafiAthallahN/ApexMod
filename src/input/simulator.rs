use crate::input::acc_udp::AccCarTelemetry;

/// Synthetic telemetry generator simulating GT3 corner entry (trail braking) and exit (throttle modulation),
/// including alternating clean laps and fault events (BRAKE_DUMP / THROTTLE_SNAP) plus synchronized ACC GT3
/// vehicle telemetry (speed_kmh, gear, abs_active, tc_active, lap_index, lap_time_ms).
pub struct CornerSimulator {
    elapsed_sec: f32,
    lap_cycle: u32,
    lap_elapsed_ms: f32,
}

impl Default for CornerSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl CornerSimulator {
    pub fn new() -> Self {
        Self {
            elapsed_sec: 0.0,
            lap_cycle: 0,
            lap_elapsed_ms: 0.0,
        }
    }

    pub fn sample(&mut self, dt: f32) -> (f32, f32) {
        let (brk, thr, _) = self.sample_with_car(dt);
        (brk, thr)
    }

    pub fn sample_with_car(&mut self, dt: f32) -> (f32, f32, AccCarTelemetry) {
        self.elapsed_sec += dt;
        self.lap_elapsed_ms += dt * 1000.0;

        let cycle_duration = 5.0; // 5-second corner sequence
        if self.elapsed_sec >= cycle_duration {
            self.elapsed_sec -= cycle_duration;
            self.lap_cycle = self.lap_cycle.wrapping_add(1);
            if self.lap_cycle.is_multiple_of(4) {
                self.lap_elapsed_ms = 0.0;
            }
        }

        let t = self.elapsed_sec;
        let induce_fault = self.lap_cycle % 2 == 1;

        // Corner phases across 5.0s:
        // 0.0 - 0.8s: Full throttle straight (1.0) -> lift off
        // 0.8 - 1.1s: Initial heavy brake application (0.0 -> 0.95)
        // 1.1 - 2.4s: Trail-braking decay toward apex (smooth progressive decay vs continuous BRAKE_DUMP at t=1.45s)
        // 2.4 - 2.7s: Mid-corner coast / apex transition
        // 2.7 - 4.2s: Corner exit throttle ramp (progressive ramp vs continuous THROTTLE_SNAP at t=2.85s)
        // 4.2 - 5.0s: Full throttle exit straight (1.0)
        let mut abs_active = false;
        let mut tc_active = false;

        let brk = if (0.8..1.1).contains(&t) {
            let p = (t - 0.8) / 0.3;
            (p * 0.95).clamp(0.0, 1.0)
        } else if (1.1..2.4).contains(&t) {
            if induce_fault {
                if t < 1.45 {
                    let p = (t - 1.1) / 0.35;
                    (0.95 - p * 0.13).clamp(0.0, 1.0)
                } else if t < 1.53 {
                    abs_active = true;
                    let p = (t - 1.45) / 0.08;
                    (0.82 * (1.0 - p)).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                let p = (t - 1.1) / 1.3;
                (0.95 * (1.0 - p).powf(1.6)).clamp(0.0, 1.0)
            }
        } else {
            0.0
        };

        let thr = if t < 0.7 {
            1.0
        } else if (0.7..0.8).contains(&t) {
            1.0 - ((t - 0.7) / 0.1)
        } else if (2.7..4.2).contains(&t) {
            if induce_fault {
                if t < 2.85 {
                    let p = (t - 2.7) / 0.15;
                    (p * 0.14).clamp(0.0, 1.0)
                } else if t < 2.92 {
                    tc_active = true;
                    let p = (t - 2.85) / 0.07;
                    (0.14 + p * 0.81).clamp(0.0, 1.0)
                } else {
                    if t < 3.15 {
                        tc_active = true;
                    }
                    let p = (t - 2.92) / (4.2 - 2.92);
                    (0.95 + p * 0.05).clamp(0.0, 1.0)
                }
            } else {
                let p = (t - 2.7) / 1.5;
                (p.powf(1.35)).clamp(0.0, 1.0)
            }
        } else if t >= 4.2 {
            1.0
        } else {
            0.0
        };

        // Synthesize realistic Monza T1 Rettifilo GT3 speed (km/h) & sequential gearbox (1..6)
        let speed_kmh = if t < 0.8 {
            252.0 + (t / 0.8) * 16.0 // 252 -> 268 km/h approaching braking board
        } else if t < 2.4 {
            let p = ((t - 0.8) / 1.6).clamp(0.0, 1.0);
            268.0 - p * 182.0 // Decelerating 268 -> 86 km/h into apex
        } else if t < 2.7 {
            86.0 - ((t - 2.4) / 0.3) * 4.0 // 82 km/h minimum apex speed
        } else {
            let p = ((t - 2.7) / 2.3).clamp(0.0, 1.0);
            82.0 + p * 170.0 // Accelerating 82 -> 252 km/h down Curva Grande
        };

        let gear = if speed_kmh > 235.0 {
            6
        } else if speed_kmh > 195.0 {
            5
        } else if speed_kmh > 155.0 {
            4
        } else if speed_kmh > 118.0 {
            3
        } else if speed_kmh > 75.0 {
            2
        } else {
            1
        };

        let lap_index = ((self.lap_cycle / 4) + 1) as i32;

        let car = AccCarTelemetry {
            speed_kmh: (speed_kmh * 10.0).round() / 10.0,
            gear,
            abs_active,
            tc_active,
            lap_index,
            lap_time_ms: self.lap_elapsed_ms as u32,
            track_id: "monza".to_string(),
            car_model: "porsche_992_gt3_r".to_string(),
            udp_connected: true,
        };

        (brk, thr, car)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CalibrationProfile;
    use crate::physics::{ModulationEvaluator, ModulationEvent};

    #[test]
    fn clean_lap_vs_fault_lap_simulation_integrates_with_ema_physics() {
        let mut sim = CornerSimulator::new();
        let mut eval = ModulationEvaluator::new();
        let profile = CalibrationProfile::default(); // Uses production EMA alpha = 0.35

        let dt = 0.004; // 250Hz
        let mut clean_lap_events = Vec::new();
        // Run first 5.0s lap (clean lap: lap_cycle == 0)
        for step in 0..1250 {
            let (brk, thr, car) = sim.sample_with_car(dt);
            let frame = eval.step(brk, thr, dt, &profile, step * 4, true, &car);
            if let Some(ev) = frame.event {
                clean_lap_events.push(ev);
            }
        }
        assert!(
            clean_lap_events.is_empty(),
            "Expected zero faults on clean progressive lap, got {:?}",
            clean_lap_events
        );

        // Run second 5.0s lap (fault lap: lap_cycle == 1)
        let mut fault_lap_events = Vec::new();
        for step in 1250..2500 {
            let (brk, thr, car) = sim.sample_with_car(dt);
            let frame = eval.step(brk, thr, dt, &profile, step * 4, true, &car);
            if let Some(ev) = frame.event {
                fault_lap_events.push(ev);
            }
        }
        assert!(
            fault_lap_events.contains(&ModulationEvent::BrakeDump),
            "Expected BrakeDump on fault lap, got {:?}",
            fault_lap_events
        );
        assert!(
            fault_lap_events.contains(&ModulationEvent::ThrottleSnap),
            "Expected ThrottleSnap on fault lap, got {:?}",
            fault_lap_events
        );
    }
}
