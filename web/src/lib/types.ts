export type ModulationEventType = 'BRAKE_DUMP' | 'THROTTLE_SNAP' | null;

export interface TelemetryFrame {
  t: number;
  brk: number;
  thr: number;
  d_brk: number;
  d_thr: number;
  j_brk?: number;
  j_thr?: number;
  event: ModulationEventType;
  score: number;
  connected?: boolean;
  device_name?: string;
  speed_kmh: number;
  gear: number;
  abs_active: boolean;
  tc_active: boolean;
  lap_index: number;
  lap_time_ms: number;
  track_id: string;
  car_model: string;
  udp_connected: boolean;
}

export interface DeadzoneConfig {
  inner: number;
  outer: number;
}

export interface CalibrationProfile {
  id: string;
  device_name: string;
  brake_deadzone: DeadzoneConfig;
  throttle_deadzone: DeadzoneConfig;
  ema_alpha: number;
  dump_velocity_threshold: number;
  snap_velocity_threshold: number;
  audio_muted: boolean;
  audio_volume: number;
}
