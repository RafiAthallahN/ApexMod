#[cfg(target_os = "linux")]
use evdev::{AbsoluteAxisCode, Device, EventSummary};
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

pub struct EvdevReader {
    #[cfg(target_os = "linux")]
    device: Option<Device>,
    pub device_name: String,
    pub brake_raw: f32,
    pub throttle_raw: f32,
    brake_max: f32,
    throttle_max: f32,
    logged_permission_hint: bool,
}

impl Default for EvdevReader {
    fn default() -> Self {
        Self::new()
    }
}

impl EvdevReader {
    pub fn new() -> Self {
        let mut reader = Self {
            #[cfg(target_os = "linux")]
            device: None,
            device_name: "Uncalibrated / Disconnected".to_string(),
            brake_raw: 0.0,
            throttle_raw: 0.0,
            brake_max: 255.0,
            throttle_max: 255.0,
            logged_permission_hint: false,
        };
        reader.try_connect();
        reader
    }

    pub fn is_connected(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.device.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub fn try_connect(&mut self) -> bool {
        #[cfg(target_os = "linux")]
        {
            if self.device.is_some() {
                return true;
            }

            let mut total_enumerated = 0usize;
            for (_path, dev) in evdev::enumerate() {
                total_enumerated += 1;
                let supported_abs = dev.supported_absolute_axes();
                let has_triggers = supported_abs.is_some_and(|axes| {
                    axes.contains(AbsoluteAxisCode::ABS_Z)
                        || axes.contains(AbsoluteAxisCode::ABS_RZ)
                        || axes.contains(AbsoluteAxisCode::ABS_BRAKE)
                        || axes.contains(AbsoluteAxisCode::ABS_GAS)
                });

                if has_triggers {
                    let name = dev.name().unwrap_or("Linux evdev Gamepad").to_string();
                    if let Ok(abs_state) = dev.get_abs_state() {
                        for (idx, info) in abs_state.iter().enumerate() {
                            let code = AbsoluteAxisCode(idx as u16);
                            if (code == AbsoluteAxisCode::ABS_Z
                                || code == AbsoluteAxisCode::ABS_BRAKE)
                                && info.maximum > 0
                            {
                                self.brake_max = info.maximum as f32;
                            }
                            if (code == AbsoluteAxisCode::ABS_RZ
                                || code == AbsoluteAxisCode::ABS_GAS)
                                && info.maximum > 0
                            {
                                self.throttle_max = info.maximum as f32;
                            }
                        }
                    }

                    // Set non-blocking mode on the underlying file descriptor for 250Hz polling
                    let fd = dev.as_raw_fd();
                    unsafe {
                        let flags = libc::fcntl(fd, libc::F_GETFL);
                        if flags >= 0 {
                            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                        }
                    }

                    tracing::info!(
                        "Connected to evdev gamepad: {} (brake_max={}, throttle_max={})",
                        name,
                        self.brake_max,
                        self.throttle_max
                    );
                    self.device_name = name;
                    self.device = Some(dev);
                    return true;
                }
            }

            // Diagnostic check if /dev/input/event* nodes exist but are unreadable due to permissions
            if !self.logged_permission_hint {
                self.logged_permission_hint = true;
                if let Ok(entries) = std::fs::read_dir("/dev/input") {
                    let mut perm_denied_count = 0usize;
                    for entry in entries.flatten() {
                        let fname = entry.file_name();
                        if fname.to_string_lossy().starts_with("event") {
                            if let Err(e) = std::fs::File::open(entry.path()) {
                                if e.kind() == std::io::ErrorKind::PermissionDenied {
                                    perm_denied_count += 1;
                                }
                            }
                        }
                    }
                    if perm_denied_count > 0 && total_enumerated == 0 {
                        tracing::warn!(
                            "Diagnostics: {} /dev/input/event* nodes returned PermissionDenied. \
                             Add your user to the 'input' group (`sudo usermod -aG input $USER`) \
                             or enable --sim mode.",
                            perm_denied_count
                        );
                        self.device_name =
                            "Uncalibrated (PermissionDenied on /dev/input/event*)".to_string();
                    }
                }
            }
            false
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub fn poll(&mut self) -> (f32, f32, bool) {
        #[cfg(target_os = "linux")]
        {
            let mut disconnected_err = None;
            if let Some(dev) = self.device.as_mut() {
                match dev.fetch_events() {
                    Ok(iterator) => {
                        for ev in iterator {
                            if let EventSummary::AbsoluteAxis(_, axis, value) = ev.destructure() {
                                match axis {
                                    AbsoluteAxisCode::ABS_Z | AbsoluteAxisCode::ABS_BRAKE => {
                                        self.brake_raw =
                                            (value as f32 / self.brake_max).clamp(0.0, 1.0);
                                    }
                                    AbsoluteAxisCode::ABS_RZ | AbsoluteAxisCode::ABS_GAS => {
                                        self.throttle_raw =
                                            (value as f32 / self.throttle_max).clamp(0.0, 1.0);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        return (self.brake_raw, self.throttle_raw, true);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        return (self.brake_raw, self.throttle_raw, true);
                    }
                    Err(e) => {
                        disconnected_err = Some(e);
                    }
                }
            }
            if let Some(e) = disconnected_err {
                tracing::warn!("Gamepad disconnected ({}): {}", self.device_name, e);
                self.device = None;
                self.device_name = "Uncalibrated / Disconnected".to_string();
                self.brake_raw = 0.0;
                self.throttle_raw = 0.0;
            }
            (0.0, 0.0, false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            (0.0, 0.0, false)
        }
    }
}
