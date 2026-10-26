pub mod linux_evdev;
pub mod simulator;

use linux_evdev::EvdevReader;
use simulator::CornerSimulator;

pub struct InputManager {
    evdev: EvdevReader,
    simulator: CornerSimulator,
    pub simulation_enabled: bool,
    reconnect_timer: f32,
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new(false)
    }
}

impl InputManager {
    pub fn new(simulation_enabled: bool) -> Self {
        Self {
            evdev: EvdevReader::new(),
            simulator: CornerSimulator::new(),
            simulation_enabled,
            reconnect_timer: 0.0,
        }
    }

    /// Poll at 250Hz. Returns (raw_brake, raw_throttle, connected, device_name).
    pub fn poll(&mut self, dt: f32) -> (f32, f32, bool, String) {
        if !self.evdev.is_connected() {
            self.reconnect_timer += dt;
            if self.reconnect_timer >= 2.0 {
                self.reconnect_timer = 0.0;
                self.evdev.try_connect();
            }
        }

        let (brk, thr, hw_connected) = self.evdev.poll();
        if hw_connected {
            return (brk, thr, true, self.evdev.device_name.clone());
        }

        if self.simulation_enabled {
            let (sim_brk, sim_thr) = self.simulator.sample(dt);
            return (
                sim_brk,
                sim_thr,
                true,
                "ApexMod Virtual GT3 Telemetry Rig".to_string(),
            );
        }

        (0.0, 0.0, false, "Uncalibrated / Disconnected".to_string())
    }
}
