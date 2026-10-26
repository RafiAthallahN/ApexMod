# ApexMod
> **Gamepad Trigger Modulation & Real-Time Telemetry Companion for Sim Racing**

ApexMod is a high-performance, non-intrusive background daemon and secondary-screen telemetry dashboard engineered for competitive sim racers using gamepads (Xbox, PlayStation DualSense, Hall-effect, and third-party controllers). It samples analog trigger inputs natively at $250\text{ Hz}$ to calculate instantaneous trigger velocities ($d(\text{input})/dt$) and jerk ($d^2(\text{input})/dt^2$). Through real-time directional stereo audio earcons ($< 15\text{ ms}$ latency) and a $120\text{ FPS}$ HTML5 Canvas visual ribbon, ApexMod trains muscle memory for progressive trail braking and smooth corner-exit throttle application without anti-cheat conflicts or direct game process injection.

---

## Table of Contents

- [Key Features](#key-features)
- [Tech Stack](#tech-stack)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
  - [1. Clone the Repository](#1-clone-the-repository)
  - [2. System Dependencies & Controller Permissions](#2-system-dependencies--controller-permissions)
  - [3. Build Frontend Assets](#3-build-frontend-assets)
  - [4. Build & Run the Rust Core Daemon](#4-build--run-the-rust-core-daemon)
  - [5. Accessing the Dashboard](#5-accessing-the-dashboard)
- [Connecting with Sim Racing Games](#connecting-with-sim-racing-games)
- [Architecture & Deep System Design](#architecture--deep-system-design)
  - [Directory Structure](#directory-structure)
  - [Execution Flow & Threading Model](#execution-flow--threading-model)
  - [Data Pipeline & Math Formulations](#data-pipeline--math-formulations)
  - [Audio Synthesis Engine (Earcons)](#audio-synthesis-engine-earcons)
  - [WebSocket & REST Network Layer](#websocket--rest-network-layer)
  - [Database Schema (SQLite WAL)](#database-schema-sqlite-wal)
- [Configuration & Environment Variables](#configuration--environment-variables)
- [Available Scripts & Commands](#available-scripts--commands)
- [Testing & Quality Verification](#testing--quality-verification)
- [Production & Background Daemon Setup](#production--background-daemon-setup)
- [Troubleshooting](#troubleshooting)
  - [1. Phone Cannot Connect ("Site can't be reached")](#1-phone-cannot-connect-site-cant-be-reached)
  - [2. Gamepad Shows "Uncalibrated / PermissionDenied"](#2-gamepad-shows-uncalibrated--permissiondenied)
  - [3. Audio Cues Are Silent or Choppy](#3-audio-cues-are-silent-or-choppy)
  - [4. Frontend Vite Build Fails](#4-frontend-vite-build-fails)
- [License](#license)

---

## Key Features

- **$250\text{ Hz}$ Native Input Sampling:** Reads raw analog trigger displacement via Linux kernel `evdev` (`/dev/input/event*`), completely bypassing X11/Wayland event queues and Wine/Proton virtualization layers.
- **Physics Derivative Processing:**
  - **Brake Dump Detection:** Automatically flags `BRAKE_DUMP` when releasing heavy brake pressure ($> 70\%$) faster than configurable decay thresholds ($V_{\text{dump}} \ge 150\%/\text{s}$).
  - **Throttle Snap Detection:** Detects aggressive throttle spikes on corner exits ($V_{\text{snap}} \ge 180\%/\text{s}$).
  - **Modulation Index Scoring:** Computes continuous $0\text{–}100\%$ driver smoothness scores based on jerk ($d^2(\text{input})/dt^2$).
- **Sub-15ms Spatial Earcon Audio Synth:** In-memory procedural stereo FM synthesis:
  - **Left Channel (Brake):** Emits smooth descending tones proportional to trail-braking bleed-off rate, triggering a dissonant $300\text{ Hz}$ downward chirp upon brake dumping.
  - **Right Channel (Throttle):** Emits an ascending harmonic tone during throttle roll-on, triggering a sharp $2.4\text{ kHz}$ warning beep upon throttle snapping.
- **$120\text{ FPS}$ Secondary Telemetry Dashboard:** Zero-DOM-overhead HTML5 Canvas 2D scrolling ribbon showing the last 3.0 seconds of input against optimal progressive decay target envelopes.
- **Zero Anti-Cheat & Compositor Conflicts:** Operates as a parallel hardware listener. Never injects code or reads memory from game binaries (EAC, BattlEye, and Proton safe).
- **Embedded Web & WebSocket Server:** Runs on port `3000` (Axum) with an independent high-refresh WebSocket stream on port `8765` (`tokio-tungstenite`), accessible from any smartphone, tablet, or secondary monitor on LAN.
- **Built-in GT3 Virtual Rig Simulator:** `--sim` flag generates authentic cornering cycles (heavy straight-line braking, trail braking decay, apex coast, and throttle exit) for calibration without requiring a physical controller plugged in.

---

## Tech Stack

- **Backend Daemon:** Rust (Edition 2021)
  - **Async Runtime:** `tokio` (v1.43)
  - **Input Subsystem:** `evdev` (v0.13, Linux kernel event interface)
  - **Web Framework:** `axum` (v0.8) with `tower-http` (static file serving & CORS)
  - **WebSocket Broadcast:** `tokio-tungstenite` (v0.26)
  - **Audio Engine:** Dynamic low-latency FFI via `libloading` with PulseAudio / PipeWire (`libpulse-simple.so.0`)
  - **Persistence:** `rusqlite` (v0.33) with Write-Ahead Logging (`WAL`)
  - **Serialization:** `serde` & `serde_json`
- **Frontend Dashboard:** Svelte 5 + TypeScript + Vite 6
  - **Rendering:** Native HTML5 Canvas 2D API (`requestAnimationFrame`)
  - **Styling:** Design System v1.0 (Obsidian Cockpit palette, 8px grid tokens, responsive compact/expanded breakpoints)
  - **Fonts:** Inter & JetBrains Mono

---

## Prerequisites

Before building ApexMod, ensure the following toolchains and libraries are present on your system:

| Dependency | Minimum Version | Installation Command (Ubuntu / Debian) |
| :--- | :--- | :--- |
| **Rust & Cargo** | `1.75+` (tested on `1.97+`) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Node.js** | `v20+` (tested on `v24+`) | `curl -fsSL https://deb.nodesource.com/setup_20.x \| sudo -E bash - && sudo apt install -y nodejs` |
| **PulseAudio / PipeWire** | Runtime library | `sudo apt install -y libpulse0` |
| **SQLite 3** | `3.35+` | `sudo apt install -y sqlite3 libsqlite3-dev` |
| **Build Essentials** | `gcc`, `pkg-config` | `sudo apt install -y build-essential pkg-config` |

---

## Getting Started

### 1. Clone the Repository

```bash
git clone https://github.com/your-username/apex-mod.git
cd apex-mod
```

### 2. System Dependencies & Controller Permissions

On Linux, raw gamepad event nodes located in `/dev/input/event*` are owned by `root:input` with `0660` permissions. To allow ApexMod to read controller triggers without running as `root`:

```bash
# Add your current user to the input group
sudo usermod -aG input $USER

# Apply group membership immediately to your current shell
newgrp input
```

### 3. Build Frontend Assets

The Rust binary embeds and serves the compiled web dashboard from `web/dist`. Build the Svelte 5 frontend first:

```bash
cd web
npm install
npm run build
cd ..
```

*Expected output: Vite compiles `dist/index.html` and bundled JS/CSS assets in `< 500ms`.*

### 4. Build & Run the Rust Core Daemon

#### Mode A: Virtual GT3 Telemetry Rig (Testing & Calibration)
To test the visual ribbon and audio synthesis without a physical controller plugged in:

```bash
cargo run -- --sim
```

#### Mode B: Physical Gamepad Mode (Sim Racing)
To ingest physical controller hardware triggers:

```bash
cargo run
```

### 5. Accessing the Dashboard

Once the daemon starts, it listens on all network interfaces (`0.0.0.0`):

- **Local PC:** Open [http://localhost:3000](http://localhost:3000)
- **Secondary Screen / Phone / Tablet on LAN:** Open `http://<YOUR-PC-IP>:3000` (e.g., `http://192.168.0.113:3000`).

---

## Connecting with Sim Racing Games

ApexMod functions entirely **out-of-band**. You do **not** need to install plugins or launch games inside specific wrappers.

```
┌─────────────────────────────────────────┐
│        Gamepad (Xbox / DualSense)       │
└───────────────────┬─────────────────────┘
                    │ USB / Bluetooth kernel events
          ┌─────────┴─────────┐
          ▼                   ▼
┌──────────────────┐ ┌───────────────────────────────────────┐
│ ApexMod Daemon   │ │ Sim Racing Title                      │
│ (250Hz Audio &   │ │ (Assetto Corsa Competizione, F1 24,   │
│  LAN Telemetry)  │ │  iRacing, Automobilista 2, Le Mans)   │
└──────────────────┘ └───────────────────────────────────────┘
```

1. **Launch ApexMod:** Run `cargo run` in your terminal.
2. **Launch your Game:** Start your racing title via Steam, Proton, or native launcher.
3. **Verify Trigger Feedback:**
   - Both your game and ApexMod read from `/dev/input/event*` simultaneously without input lock or contention.
   - Squeeze the brake trigger: as you bleed off the pedal into corner apexes, listen for the pitch descending smoothly in your left ear.
   - If you release too abruptly (causing front-end load transfer shock and understeer), ApexMod plays a downward warning chirp and pulses the red dashboard banner.

---

## Architecture & Deep System Design

### Directory Structure

```
apex-mod/
├── Cargo.toml               # Rust workspace & dependencies
├── src/
│   ├── main.rs              # Daemon entry point, 250Hz core loop & server joins
│   ├── config.rs            # Calibration profiles, deadzones, thresholds & EMA alpha
│   ├── audio/
│   │   └── mod.rs           # Procedural stereo FM synthesis engine (libpulse-simple FFI)
│   ├── input/
│   │   ├── mod.rs           # InputManager orchestrating hardware & simulation sources
│   │   ├── linux_evdev.rs   # Non-blocking Linux /dev/input/event* trigger reader
│   │   └── simulator.rs     # Realistic 5.0s GT3 corner cycle generator
│   ├── physics/
│   │   └── mod.rs           # EMA filter, 1st/2nd derivative evaluator & Modulation Index
│   ├── server/
│   │   └── mod.rs           # Axum HTTP server, REST endpoints & WebSocket broadcasters
│   └── storage/
│       └── mod.rs           # SQLite 3 WAL store for profiles and event history
└── web/                     # Svelte 5 + Vite telemetry frontend
    ├── package.json
    ├── vite.config.ts
    ├── index.html
    └── src/
        ├── app.css          # Design System v1.0 tokens, typography & spatial grid
        ├── App.svelte       # Cockpit layout, WebSocket client & breakpoint manager
        └── lib/
            ├── types.ts     # TelemetryFrame & CalibrationProfile definitions
            └── components/
                ├── LiveInputRibbon.svelte   # Component A: 120 FPS HTML5 Canvas 2D ribbon
                ├── RadialJerkGauge.svelte   # Component B: Derivative needle & Jerk HUD
                └── EventAlertBanner.svelte  # Component C: 400ms pulsing event capsule
```

### Execution Flow & Threading Model

ApexMod separates real-time mathematical guarantees from network and UI I/O using dedicated system threads:

```
┌────────────────────────────────────────────────────────────────────────┐
│                        APEXMOD BACKEND DAEMON                          │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │ Thread 1: 250 Hz Input & Physics Core (4.0 ms deterministic tick)│  │
│  │                                                                  │  │
│  │  [evdev Reader] ──> [Deadzone Norm] ──> [1st-order EMA Filter]   │  │
│  │                                                │                 │  │
│  │                                                ▼                 │  │
│  │                                  [Derivative & Jerk Evaluator]   │  │
│  │                                                │                 │  │
│  │                       ┌────────────────────────┴────────┐        │  │
│  │                       ▼                                 ▼        │  │
│  │              [Event Detection]               [Modulation Score]  │  │
│  └───────────────────────┬─────────────────────────────────┬────────┘  │
│                          │ State Update                    │ 60Hz Tick │
│                          ▼                                 ▼           │
│  ┌────────────────────────────────┐     ┌───────────────────────────┐  │
│  │ Thread 2: Earcon Audio Synth   │     │ Async Task: tokio Runtime │  │
│  │ (libpulse-simple, 48kHz, 4ms)  │     │                           │  │
│  │ - Left: Brake decay & chirp    │     │ • ws://0.0.0.0:8765       │  │
│  │ - Right: Throttle ramp & beep  │     │ • http://0.0.0.0:3000     │  │
│  └────────────────────────────────┘     │ • SQLite WAL Persistence  │  │
│                                         └───────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

### Data Pipeline & Math Formulations

1. **Deadzone Normalization:**
   Raw inputs $[x_{\min}, x_{\max}]$ are clamped and mapped across inner and outer deadzone thresholds:
   $$\text{Norm}(x) = \operatorname{clamp}\left(\frac{\operatorname{clamp}(x, dz_{\text{in}}, dz_{\text{out}}) - dz_{\text{in}}}{dz_{\text{out}} - dz_{\text{in}}}, 0.0, 1.0\right)$$

2. **Exponential Moving Average (EMA) Filter:**
   Suppresses mechanical potentiometer jitter without introducing phase delay:
   $$y_t = \alpha \cdot x_t + (1 - \alpha) \cdot y_{t-1} \quad (\text{default } \alpha = 0.35)$$

3. **First & Second Derivatives:**
   $$\text{Velocity } v_t = \frac{y_t - y_{t-1}}{\Delta t}, \quad \text{Jerk } j_t = \frac{v_t - v_{t-1}}{\Delta t}$$

4. **Event Detection Logic:**
   - **`BrakeDump`:** Triggered when the driver held $> 70\%$ brake within the past $350\text{ ms}$ and executes a release rate $v_{\text{brk}} < -V_{\text{dump\_threshold}}$ (default $-150\%/\text{s}$).
   - **`ThrottleSnap`:** Triggered when transitioning from low throttle ($< 20\%$) and spikes $v_{\text{thr}} > +V_{\text{snap\_threshold}}$ (default $+180\%/\text{s}$).

5. **Modulation Index ($0\text{–}100\%$):**
   Calculates driver input smoothness dynamically as a function of instantaneous jerk:
   $$\text{Score}_t = \text{Score}_{t-1} \cdot (1 - \beta) + \left(100.0 - \frac{|j_{\text{brk}}| \cdot 0.6 + |j_{\text{thr}}| \cdot 0.4}{60.0}\right) \cdot \beta$$

### Audio Synthesis Engine (Earcons)

ApexMod avoids audio file I/O latency by generating procedural raw PCM float32 buffers directly in RAM.
- **Latency:** $\approx 4\text{ ms}$ buffer chunks ($192$ samples at $48\text{ kHz}$) written via non-blocking PulseAudio simple stream.
- **Stereo Isolation:** Left = Brake Channel, Right = Throttle Channel.
- **Idle Gating:** When inputs are stationary, the audio loop sleeps without streaming empty buffers, keeping CPU usage below $0.1\%$.

### WebSocket & REST Network Layer

- **Broadcast Rate:** $60\text{ Hz}$ broadcast ticker.
- **Payload Schema:**
  ```json
  {
    "t": 1698000000123,
    "brk": 0.421,
    "thr": 0.000,
    "d_brk": -0.842,
    "d_thr": 0.000,
    "j_brk": 12.4,
    "j_thr": 0.0,
    "event": "BRAKE_DUMP",
    "score": 88.5,
    "connected": true,
    "device_name": "Microsoft Xbox Wireless Controller"
  }
  ```

### Database Schema (SQLite WAL)

Stored in local file `apexmod.db` with `PRAGMA journal_mode = WAL;` and `PRAGMA synchronous = NORMAL;`:

```sql
CREATE TABLE IF NOT EXISTS calibration_profiles (
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
);
```

---

## Configuration & Environment Variables

### CLI Options

| Argument | Description | Default |
| :--- | :--- | :--- |
| `--sim`, `--simulate` | Launch with the synthetic GT3 Corner Simulator rig active | `false` |

### Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `APEXMOD_SIM` | Enable simulator mode via environment (`1` or `true`) | `0` |
| `RUST_LOG` | Logging verbosity filter (e.g., `apex_mod=debug`, `info`) | `apex_mod=info` |

---

## Available Scripts & Commands

### Rust Daemon (`Cargo`)

| Command | Description |
| :--- | :--- |
| `cargo run` | Compile and run ApexMod in hardware gamepad mode |
| `cargo run -- --sim` | Run daemon in virtual GT3 simulator mode |
| `cargo test` | Execute unit and integration tests |
| `cargo clippy --all-targets -- -D warnings` | Run strict linter across all targets |
| `cargo build --release` | Produce optimized static binary at `target/release/apex-mod` |

### Web Frontend (`npm` inside `web/`)

| Command | Description |
| :--- | :--- |
| `npm run dev` | Start Vite dev server on `http://localhost:5173` with HMR and backend proxy |
| `npm run build` | Compile Svelte 5 application into production static assets (`web/dist`) |
| `npx tsc --noEmit` | Validate TypeScript types |

---

## Testing & Quality Verification

ApexMod includes deterministic unit and integration test suites:

```bash
# Run backend test suite
cargo test
```

### Key Tests

1. `physics::tests::detects_brake_dump_and_throttle_snap`: Verifies that sudden transitions trigger corresponding fault events and threshold penalties.
2. `input::simulator::tests::clean_lap_vs_fault_lap_simulation_integrates_with_ema_physics`: Verifies that a clean simulated lap generates **zero** false-positive fault events with EMA filtering enabled, and that fault laps reliably trigger both `BrakeDump` and `ThrottleSnap`.

---

## Production & Background Daemon Setup

To run ApexMod as a persistent user service that starts automatically upon boot:

1. Create a systemd user unit file:
   ```bash
   mkdir -p ~/.config/systemd/user
   nano ~/.config/systemd/user/apexmod.service
   ```

2. Paste the following configuration:
   ```ini
   [Unit]
   Description=ApexMod Sim Racing Telemetry Daemon
   After=sound.target

   [Service]
   Type=simple
   WorkingDirectory=%h/Documents/rnd/tauri/apex-mod
   ExecStart=%h/Documents/rnd/tauri/apex-mod/target/release/apex-mod
   Restart=always
   RestartSec=3

   [Install]
   WantedBy=default.target
   ```

3. Enable and start the service:
   ```bash
   # Build release binary first
   cargo build --release

   # Reload systemd and start service
   systemctl --user daemon-reload
   systemctl --user enable --now apexmod.service

   # Check live status
   systemctl --user status apexmod.service
   ```

---

## Troubleshooting

### 1. Phone Cannot Connect ("Site can't be reached")

- **Explicitly use `http://` (not `https://`):**
  Mobile browsers (Safari/Chrome) often auto-prefix IP addresses with `https://`. Since ApexMod runs an unencrypted local dashboard, navigating to `https://192.168.0.x:3000` will fail. Use:
  ```text
  http://192.168.0.113:3000
  ```
- **Allow Port in Linux Firewall (`ufw`):**
  If `ufw` is active on Linux, incoming connections from other devices are blocked by default:
  ```bash
  sudo ufw allow 3000/tcp
  sudo ufw allow 8765/tcp
  ```
- **Wi-Fi Router AP Isolation (Client Isolation):**
  If you cannot ping your phone from your PC (`ping <PHONE-IP>`), your Wi-Fi router has Client Isolation turned on, preventing devices from communicating.
  - **Quick Bypass:** Run an instant tunnel on your PC:
    ```bash
    ssh -R 80:localhost:3000 nokey@localhost.run
    ```
    Open the generated URL on your phone.
  - **Ultra Low Latency Solution:** Connect your phone via USB cable and toggle **USB Tethering** on in Android/iOS settings.

### 2. Gamepad Shows "Uncalibrated / PermissionDenied"

If the dashboard displays `Uncalibrated (PermissionDenied on /dev/input/event*)`:
Your user account does not have read permissions for Linux input device nodes.
```bash
sudo usermod -aG input $USER
newgrp input
```

### 3. Audio Cues Are Silent or Choppy

- Ensure PulseAudio or PipeWire is running:
  ```bash
  pactl info
  ```
- Verify `libpulse-simple.so.0` is present on your system:
  ```bash
  find /usr/lib* -name "libpulse-simple.so*"
  ```
- Unmute in the UI: Click the **`EARCONS: MUTED`** button in the top navigation bar to toggle audio feedback.

### 4. Frontend Vite Build Fails

Ensure Node.js is version 20 or higher:
```bash
node -v
cd web
rm -rf node_modules package-lock.json
npm install
npm run build
```

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
