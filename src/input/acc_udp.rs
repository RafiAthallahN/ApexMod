use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccCarTelemetry {
    pub speed_kmh: f32,
    pub gear: i32,
    pub abs_active: bool,
    pub tc_active: bool,
    pub lap_index: i32,
    pub lap_time_ms: u32,
    pub track_id: String,
    pub car_model: String,
    pub udp_connected: bool,
}

impl Default for AccCarTelemetry {
    fn default() -> Self {
        Self {
            speed_kmh: 0.0,
            gear: 0,
            abs_active: false,
            tc_active: false,
            lap_index: 1,
            lap_time_ms: 0,
            track_id: "monza".to_string(),
            car_model: "porsche_992_gt3_r".to_string(),
            udp_connected: false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JsonUdpTelemetryPacket {
    speed_kmh: Option<f32>,
    gear: Option<i32>,
    abs_active: Option<u8>,
    tc_active: Option<u8>,
    lap_index: Option<i32>,
    lap_time_ms: Option<u32>,
    track_id: Option<String>,
    car_model: Option<String>,
}

pub struct AccUdpReceiver {
    socket: Option<UdpSocket>,
    pub state: AccCarTelemetry,
    last_packet_at: Option<Instant>,
    pub port: u16,
}

impl Default for AccUdpReceiver {
    fn default() -> Self {
        Self::new(9000)
    }
}

impl AccUdpReceiver {
    pub fn new(port: u16) -> Self {
        let bind_addr = format!("127.0.0.1:{}", port);
        let socket = match UdpSocket::bind(&bind_addr) {
            Ok(sock) => {
                let _ = sock.set_nonblocking(true);
                tracing::info!("ACC UDP telemetry listener bound to udp://{}", bind_addr);
                Some(sock)
            }
            Err(e) => {
                tracing::warn!(
                    "Could not bind ACC UDP listener on {} ({}); trying ephemeral client socket",
                    bind_addr,
                    e
                );
                if let Ok(sock) = UdpSocket::bind("127.0.0.1:0") {
                    let _ = sock.set_nonblocking(true);
                    let receiver = Self {
                        socket: Some(sock),
                        state: AccCarTelemetry::default(),
                        last_packet_at: None,
                        port,
                    };
                    receiver.send_acc_register_handshake("asd");
                    return receiver;
                }
                None
            }
        };

        Self {
            socket,
            state: AccCarTelemetry::default(),
            last_packet_at: None,
            port,
        }
    }

    /// Sends ACC Broadcasting Protocol v4 registration packet (Command 1) to 127.0.0.1:9000
    pub fn send_acc_register_handshake(&self, connection_password: &str) {
        let Some(sock) = &self.socket else {
            return;
        };
        let mut pkt = Vec::with_capacity(64);
        pkt.push(1u8); // REGISTER_COMMAND_APPLICATION
        pkt.push(4u8); // Protocol version 4
        Self::write_acc_string(&mut pkt, "ApexMod");
        Self::write_acc_string(&mut pkt, connection_password);
        pkt.extend_from_slice(&16i32.to_le_bytes()); // 16ms update interval (~60Hz)
        Self::write_acc_string(&mut pkt, ""); // commandPassword
        let target = format!("127.0.0.1:{}", self.port);
        let _ = sock.send_to(&pkt, &target);
    }

    fn write_acc_string(buf: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);
    }

    pub fn poll(&mut self) -> &AccCarTelemetry {
        let Some(sock) = &self.socket else {
            return &self.state;
        };

        let mut buf = [0u8; 2048];
        while let Ok((len, _src)) = sock.recv_from(&mut buf) {
            if len == 0 {
                continue;
            }
            self.last_packet_at = Some(Instant::now());
            self.state.udp_connected = true;

            let payload = &buf[..len];
            if payload.starts_with(b"{") {
                if let Ok(parsed) = serde_json::from_slice::<JsonUdpTelemetryPacket>(payload) {
                    if let Some(v) = parsed.speed_kmh {
                        self.state.speed_kmh = v.max(0.0);
                    }
                    if let Some(g) = parsed.gear {
                        self.state.gear = g.clamp(-1, 6);
                    }
                    if let Some(a) = parsed.abs_active {
                        self.state.abs_active = a != 0;
                    }
                    if let Some(t) = parsed.tc_active {
                        self.state.tc_active = t != 0;
                    }
                    if let Some(l) = parsed.lap_index {
                        self.state.lap_index = l.max(1);
                    }
                    if let Some(lt) = parsed.lap_time_ms {
                        self.state.lap_time_ms = lt;
                    }
                    if let Some(tr) = parsed.track_id {
                        self.state.track_id = tr;
                    }
                    if let Some(cm) = parsed.car_model {
                        self.state.car_model = cm;
                    }
                }
            } else {
                // Binary ACC Broadcasting v4 RealtimeCarUpdate (MsgType == 3)
                Self::parse_binary_acc_packet(&mut self.state, payload);
            }
        }

        if let Some(last) = self.last_packet_at {
            if last.elapsed() > Duration::from_secs(3) {
                self.state.udp_connected = false;
            }
        }

        &self.state
    }

    fn parse_binary_acc_packet(state: &mut AccCarTelemetry, data: &[u8]) {
        if data.len() < 18 {
            return;
        }
        let msg_type = data[0];
        // MsgType 3 = RealtimeCarUpdate in Kunos ACC UDP protocol
        if msg_type == 3 {
            let raw_gear = data[6] as i32;
            state.gear = (raw_gear - 1).clamp(-1, 6);
            if data.len() >= 22 {
                let kmh = u16::from_le_bytes([data[20], data[21]]);
                state.speed_kmh = kmh as f32;
            }
            if data.len() >= 34 {
                let laps = u16::from_le_bytes([data[32], data[33]]);
                state.lap_index = (laps as i32).max(1);
            }
        }
    }
}
