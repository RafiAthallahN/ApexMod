pub mod acc_udp;
pub mod linux_evdev;
pub mod simulator;

use acc_udp::{AccCarTelemetry, AccUdpReceiver};
use linux_evdev::EvdevReader;
use simulator::CornerSimulator;

pub struct InputManager {
    evdev: EvdevReader,
    acc_udp: AccUdpReceiver,
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
            acc_udp: AccUdpReceiver::default(),
            simulator: CornerSimulator::new(),
            simulation_enabled,
            reconnect_timer: 0.0,
        }
    }

    /// Poll at 250Hz. Returns (raw_brake, raw_throttle, connected, device_name, acc_car_state).
    pub fn poll(&mut self, dt: f32) -> (f32, f32, bool, String, AccCarTelemetry) {
        if !self.evdev.is_connected() {
            self.reconnect_timer += dt;
            if self.reconnect_timer >= 2.0 {
                self.reconnect_timer = 0.0;
                self.evdev.try_connect();
            }
        }

        let live_car = self.acc_udp.poll().clone();
        let (brk, thr, hw_connected) = self.evdev.poll();
        if hw_connected {
            return (
                brk,
                thr,
                true,
                self.evdev.device_name.clone(),
                live_car,
            );
        }

        if self.simulation_enabled {
            let (sim_brk, sim_thr, mut sim_car) = self.simulator.sample_with_car(dt);
            // If real ACC UDP packets are arriving even while trigger sim is on, prefer live ACC car state
            if live_car.udp_connected {
                sim_car = live_car;
            }
            return (
                sim_brk,
                sim_thr,
                true,
                "ApexMod Virtual GT3 Telemetry Rig".to_string(),
                sim_car,
            );
        }

        (
            0.0,
            0.0,
            false,
            self.evdev.device_name.clone(),
            live_car,
        )
    }
}
