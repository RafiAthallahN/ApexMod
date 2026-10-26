use std::sync::{Arc, Mutex};
use crate::physics::{ModulationEvent, TelemetryFrame};

#[derive(Debug, Clone, Copy)]
pub struct AudioControlState {
    pub brk: f32,
    pub thr: f32,
    pub d_brk: f32,
    pub d_thr: f32,
    pub dump_chirp_remaining: f32,
    pub snap_click_remaining: f32,
    pub muted: bool,
    pub volume: f32,
}

impl Default for AudioControlState {
    fn default() -> Self {
        Self {
            brk: 0.0,
            thr: 0.0,
            d_brk: 0.0,
            d_thr: 0.0,
            dump_chirp_remaining: 0.0,
            snap_click_remaining: 0.0,
            muted: false,
            volume: 0.25,
        }
    }
}

pub struct EarconEngine {
    state: Arc<Mutex<AudioControlState>>,
}

impl EarconEngine {
    pub fn spawn() -> Self {
        let state = Arc::new(Mutex::new(AudioControlState::default()));
        let thread_state = Arc::clone(&state);

        std::thread::Builder::new()
            .name("apexmod-earcon-synth".to_string())
            .spawn(move || {
                run_audio_synth_thread(thread_state);
            })
            .expect("Failed to spawn EarconEngine audio thread");

        Self { state }
    }

    pub fn update(&self, frame: &TelemetryFrame, muted: bool, volume: f32) {
        if let Ok(mut guard) = self.state.lock() {
            guard.brk = frame.brk;
            guard.thr = frame.thr;
            guard.d_brk = frame.d_brk;
            guard.d_thr = frame.d_thr;
            guard.muted = muted;
            guard.volume = volume.clamp(0.0, 1.0);

            match frame.event {
                Some(ModulationEvent::BrakeDump) => {
                    guard.dump_chirp_remaining = 0.18; // 180ms dissonant downward chirp (300Hz band)
                }
                Some(ModulationEvent::ThrottleSnap) => {
                    guard.snap_click_remaining = 0.14; // 140ms 2.4kHz percussive warning beep
                }
                None => {}
            }
        }
    }
}

#[repr(C)]
struct PaSampleSpec {
    format: i32, // PA_SAMPLE_FLOAT32LE = 5
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct PaBufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

fn run_audio_synth_thread(state: Arc<Mutex<AudioControlState>>) {
    const SAMPLE_RATE: u32 = 48_000;
    const CHUNK_FRAMES: usize = 192; // 4.0ms buffer for < 10ms acoustic latency (FR-3.4)
    let dt_sample = 1.0 / (SAMPLE_RATE as f32);

    // Dynamically load libpulse-simple.so.0 so compilation succeeds without -dev headers
    let pa_lib = unsafe { libloading::Library::new("libpulse-simple.so.0") };
    let Ok(lib) = pa_lib else {
        tracing::warn!(
            "libpulse-simple.so.0 not available; running EarconEngine in silent telemetry-only mode"
        );
        return;
    };

    unsafe {
        type PaSimpleNewFn = unsafe extern "C" fn(
            server: *const i8,
            name: *const i8,
            dir: i32,
            dev: *const i8,
            stream_name: *const i8,
            ss: *const PaSampleSpec,
            map: *const std::ffi::c_void,
            attr: *const PaBufferAttr,
            error: *mut i32,
        ) -> *mut std::ffi::c_void;

        type PaSimpleWriteFn = unsafe extern "C" fn(
            s: *mut std::ffi::c_void,
            data: *const std::ffi::c_void,
            bytes: usize,
            error: *mut i32,
        ) -> i32;

        let Ok(pa_simple_new) = lib.get::<PaSimpleNewFn>(b"pa_simple_new\0") else {
            return;
        };
        let Ok(pa_simple_write) = lib.get::<PaSimpleWriteFn>(b"pa_simple_write\0") else {
            return;
        };

        let spec = PaSampleSpec {
            format: 5, // PA_SAMPLE_FLOAT32LE
            rate: SAMPLE_RATE,
            channels: 2, // Stereo: Left = Brake, Right = Throttle (FR-3.1)
        };

        // Target buffer attribute for sub-10ms latency
        let attr = PaBufferAttr {
            maxlength: (CHUNK_FRAMES * 8 * 4) as u32,
            tlength: (CHUNK_FRAMES * 8 * 2) as u32,
            prebuf: u32::MAX,
            minreq: u32::MAX,
            fragsize: u32::MAX,
        };

        let mut error: i32 = 0;
        let app_name = b"ApexMod\0";
        let stream_name = b"Earcon Telemetry Synth\0";
        let handle = pa_simple_new(
            std::ptr::null(),
            app_name.as_ptr() as *const i8,
            1, // PA_STREAM_PLAYBACK
            std::ptr::null(),
            stream_name.as_ptr() as *const i8,
            &spec,
            std::ptr::null(),
            &attr,
            &mut error,
        );

        if handle.is_null() {
            tracing::warn!(
                "Could not open PulseAudio/PipeWire stream (err={}); audio cues muted",
                error
            );
            return;
        }

        tracing::info!("EarconEngine stereo FM synth initialized at 48kHz (4ms frame buffer)");

        let mut phase_left: f32 = 0.0;
        let mut phase_right: f32 = 0.0;
        let mut buffer = [0.0f32; CHUNK_FRAMES * 2];
        let mut was_active = false;

        loop {
            let snap = {
                if let Ok(mut g) = state.lock() {
                    let s = *g;
                    let chunk_sec = (CHUNK_FRAMES as f32) * dt_sample;
                    g.dump_chirp_remaining = (g.dump_chirp_remaining - chunk_sec).max(0.0);
                    g.snap_click_remaining = (g.snap_click_remaining - chunk_sec).max(0.0);
                    s
                } else {
                    AudioControlState::default()
                }
            };

            let left_active = !snap.muted
                && snap.volume > 0.001
                && (snap.dump_chirp_remaining > 0.0 || (snap.brk > 0.05 && snap.d_brk < -0.05));
            let right_active = !snap.muted
                && snap.volume > 0.001
                && (snap.snap_click_remaining > 0.0 || (snap.thr > 0.05 && snap.d_thr > 0.05));
            let is_active = left_active || right_active;

            // Avoid burning IPC/CPU writing silent PCM buffers when neither channel is active
            if !is_active && !was_active {
                std::thread::sleep(std::time::Duration::from_millis(4));
                continue;
            }
            was_active = is_active;

            for i in 0..CHUNK_FRAMES {
                if !is_active {
                    buffer[i * 2] = 0.0;
                    buffer[i * 2 + 1] = 0.0;
                    continue;
                }

                // LEFT CHANNEL: Brake Decay & BrakeDump Chirp (FR-3.1, FR-3.2, Design.md §7)
                let left_sample = if snap.dump_chirp_remaining > 0.0 {
                    // Dissonant downward chirp centered around 300Hz rumble
                    let progress = 1.0 - (snap.dump_chirp_remaining / 0.18).clamp(0.0, 1.0);
                    let freq = 340.0 - progress * 160.0;
                    phase_left = (phase_left + std::f32::consts::TAU * freq * dt_sample)
                        % std::f32::consts::TAU;
                    let sub_harmonic = (phase_left * 1.5).sin() * 0.45;
                    (phase_left.sin() * 0.65 + sub_harmonic) * 0.70
                } else if snap.brk > 0.05 && snap.d_brk < -0.05 {
                    // Subtle descending pitch during trail-braking release
                    let freq = 180.0 + snap.brk * 220.0;
                    phase_left = (phase_left + std::f32::consts::TAU * freq * dt_sample)
                        % std::f32::consts::TAU;
                    let amp = ((-snap.d_brk) * 0.25).clamp(0.02, 0.25);
                    phase_left.sin() * amp
                } else {
                    0.0
                };

                // RIGHT CHANNEL: Throttle Ramp & ThrottleSnap 2.4kHz Warning Beep (FR-3.1, FR-3.3, Design.md §7)
                let right_sample = if snap.snap_click_remaining > 0.0 {
                    // Sharp 2.4 kHz warning beep with percussive envelope
                    let env = (snap.snap_click_remaining / 0.14).clamp(0.0, 1.0);
                    let freq = 2400.0;
                    phase_right = (phase_right + std::f32::consts::TAU * freq * dt_sample)
                        % std::f32::consts::TAU;
                    phase_right.sin() * env * 0.65
                } else if snap.thr > 0.05 && snap.d_thr > 0.05 {
                    // Clean ascending harmonic during progressive throttle ramp
                    let freq = 260.0 + snap.thr * 320.0;
                    phase_right = (phase_right + std::f32::consts::TAU * freq * dt_sample)
                        % std::f32::consts::TAU;
                    let amp = (snap.d_thr * 0.22).clamp(0.02, 0.22);
                    phase_right.sin() * amp
                } else {
                    0.0
                };

                buffer[i * 2] = (left_sample * snap.volume).clamp(-1.0, 1.0);
                buffer[i * 2 + 1] = (right_sample * snap.volume).clamp(-1.0, 1.0);
            }

            let mut write_err: i32 = 0;
            let rc = pa_simple_write(
                handle,
                buffer.as_ptr() as *const std::ffi::c_void,
                std::mem::size_of_val(&buffer),
                &mut write_err,
            );
            if rc < 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}
