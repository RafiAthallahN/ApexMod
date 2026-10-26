# ApexMod

> **Gamepad Trigger Modulation & Real-Time Telemetry Companion for Sim Racing**  
> *Version 2.0.0 — Featuring ACC UDP Car-State Ingestion, Asynchronous Ring-Buffer Persistence & Squared Minimalist Motorsport Console*

ApexMod is an instrument-grade, non-intrusive background daemon and secondary-screen telemetry console engineered for sim racers using gamepads (Xbox, PlayStation DualSense, Hall-effect, and third-party controllers). It samples analog trigger inputs natively at $250\text{ Hz}$ to calculate instantaneous trigger velocities ($d(\text{input})/dt$) and jerk ($d^2(\text{input})/dt^2$), synchronizing finger displacement with in-game physics telemetry from *Assetto Corsa Competizione* (ACC) via UDP on port `9000`.

Through real-time directional stereo earcons ($< 15\text{ ms}$ latency), a $120\text{ FPS}$ HTML5 Canvas dual-trace oscilloscope, and automated SQLite WAL timeseries archiving, ApexMod trains muscle memory for progressive trail-braking decay and smooth corner-exit throttle application without anti-cheat conflicts, game memory injection, or compositor overhead.

---

## Table of Contents

- [Key Features](#key-features)
- [Tech Stack](#tech-stack)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
  - [1. Clone the Repository](#1-clone-the-repository)
  - [2. System Dependencies & Linux Controller Permissions](#2-system-dependencies--linux-controller-permissions)
  - [3. Build Frontend Production Assets](#3-build-frontend-production-assets)
  - [4. Build & Run the Rust Core Daemon](#4-build--run-the-rust-core-daemon)
  - [5. Accessing the Dashboard](#5-accessing-the-dashboard)
- [Connecting with Assetto Corsa Competizione (ACC)](#connecting-with-assetto-corsa-competizione-acc)
  - [Configuring ACC UDP Broadcasting](#configuring-acc-udp-broadcasting)
  - [Parallel Hardware Ingestion Flow](#parallel-hardware-ingestion-flow)
- [Architecture & Deep System Design](#architecture--deep-system-design)
  - [Directory Structure](#directory-structure)
  - [Multi-Threaded Execution Model](#multi-threaded-execution-model)
  - [Data Pipeline & Mathematical Formulations](#data-pipeline--mathematical-formulations)
  - [Audio Synthesis Engine (Spatial Earcons)](#audio-synthesis-engine-spatial-earcons)
  - [Network & Transport Architecture](#network--transport-architecture)
  - [Database Schema (SQLite WAL & Async Ring Buffer)](#database-schema-sqlite-wal--async-ring-buffer)
  - [REST API Reference](#rest-api-reference)
- [Design System v2.0 (Squared Minimalist)](#design-system-v20-squared-minimalist)
- [Configuration & Environment Variables](#configuration--environment-variables)
- [Available Scripts & Commands](#available-scripts--commands)
- [Testing & Quality Verification](#testing--quality-verification)
- [Production & Background Daemon Setup (systemd)](#production--background-daemon-setup-systemd)
- [Troubleshooting](#troubleshooting)
  - [1. Mobile Phone or Tablet Cannot Connect ("Site can't be reached")](#1-mobile-phone-or-tablet-cannot-connect-site-cant-be-reached)
  - [2. Gamepad Shows "Uncalibrated / PermissionDenied"](#2-gamepad-shows-uncalibrated--permissiondenied)
  - [3. ACC UDP Telemetry Shows "STBY" Instead of "LINKED"](#3-acc-udp-telemetry-shows-stby-instead-of-linked)
  - [4. Audio Cues Are Silent or Missing](#4-audio-cues-are-silent-or-missing)
  - [5. Frontend Vite Build Fails](#5-frontend-vite-build-fails)
- [Contributing](#contributing)
- [License](#license)

---

## Key Features

- **$250\text{ Hz}$ Native Trigger Sampling:** Reads raw analog trigger values directly via the Linux kernel `evdev` interface (`/dev/input/event*`), completely bypassing X11/Wayland event queues and Wine/Proton virtualization layers.
- **Physics Derivative & Jerk Processing:**
  - **Brake Dump Detection:** Flags `BRAKE_DUMP` when releasing heavy brake pressure ($> 70\%$) faster than configurable decay thresholds ($V_{\text{dump}} \ge 150\%/\text{s}$).
  - **Throttle Snap Detection:** Flags `THROTTLE_SNAP` when aggressively spiking throttle ($V_{\text{snap}} \ge 180\%/\text{s}$) out of corner apexes.
  - **Continuous Modulation Index ($0\text{–}100\%$):** Evaluates instantaneous input smoothness dynamically from second derivative jerk ($d^2(\text{input})/dt^2$).
- **ACC UDP Vehicle State Synchronization (Port `9000`):** Ingests live telemetry packets from *Assetto Corsa Competizione* (speed in km/h, sequential gear, ABS lock status, Traction Control slip cut, lap index, lap time, track ID, and car model) to cross-reference trigger jerk against vehicle dynamics.
- **Sub-15ms Procedural Spatial Earcons:** In-memory procedural stereo FM tone generation using low-latency PulseAudio / PipeWire simple streams:
  - **Left Audio Channel (Brake):** Emits a smooth descending tone proportional to trail-braking bleed-off rate, triggering a dissonant $300\text{ Hz}$ downward chirp upon a brake dump.
  - **Right Audio Channel (Throttle):** Emits an ascending harmonic tone during throttle roll-on, triggering a sharp $2.4\text{ kHz}$ warning beep upon a throttle snap.
  - **Zero Idle Overhead:** Synth loop sleeps when inputs are stationary, maintaining $< 0.1\%$ CPU utilization.
- **Asynchronous SQLite WAL Telemetry Archiving:** High-throughput 10,000-sample bounded ring buffer decouples real-time $250\text{ Hz}$ physics polling from disk writes. Batches frames every $500\text{ ms}$ or $300$ samples inside a single transaction into `apexmod.db` (`sessions`, `laps`, and indexed timeseries `telemetry_frames`).
- **Squared Minimalist Instrument Console (v2.0):**
  - **Strict Orthogonality:** Clean $0\text{px}$ corner radius across every container, button, and badge (`border-radius: 0px !important`).
  - **Discrete 20-Segment LED Meters:** Vertical mechanical-style bars filling bottom-to-top with live tabular numeric readouts.
  - **120 FPS Dual-Trace Oscilloscope:** Zero-DOM HTML5 Canvas 2D rolling 3.0-second oscilloscope displaying simultaneous Brake (`#FF3333`) and Throttle (`#00E676`) curves with a 1px dashed hit-line.
  - **High-Density Vehicle Tiles:** Immediate glanceability for Speed (`KM/H`), Gear (`R`, `N`, `1`–`6`), Modulation Score (`%`), and Lap Index (`#01`).
- **Zero Anti-Cheat & Compositor Conflicts:** Operates strictly as a passive listener on hardware device nodes and local UDP broadcasts. Never injects code or reads memory from game processes (EAC, BattlEye, and Proton safe).
- **Built-in GT3 Virtual Rig Simulator:** `--sim` flag generates authentic GT3 cornering cycles (Monza T1 Rettifilo: deceleration from $268\text{ km/h} \rightarrow 82\text{ km/h}$, downshifts $6 \rightarrow 2$, ABS/TC intervention on fault laps) for testing without a gamepad or game running.

---

## Tech Stack

### Backend Core Daemon
- **Language & Edition:** Rust 2021
- **Async Runtime:** `tokio` (v1.43, multi-threaded)
- **Input Ingestion:** `evdev` (v0.13, Linux kernel input subsystem)
- **ACC Telemetry Ingestion:** Native UDP listener / Kunos Broadcasting Protocol v4
- **Web & REST Server:** `axum` (v0.8) with `tower-http` (static asset serving & permissive CORS)
- **WebSocket Broadcasting:** `tokio-tungstenite` (v0.26) on port `8765` + Axum `/ws` route on port `3000`
- **Audio Synthesis:** Procedural FM audio via dynamic FFI (`libloading`) binding to `libpulse-simple.so.0` (PulseAudio & PipeWire)
- **Storage & WAL Archiving:** `rusqlite` (v0.33) with Write-Ahead Logging (`WAL`)
- **Serialization:** `serde` & `serde_json`
- **Logging & Tracing:** `tracing` & `tracing-subscriber`

### Frontend Dashboard (v2.0)
- **Framework:** Svelte 5 (Runes mode: `$state`, `$derived`, `$props`)
- **Build Tool:** Vite 6 + TypeScript 5
- **Rendering Engine:** Native HTML5 2D Canvas API (`requestAnimationFrame` at up to $120\text{ FPS}$)
- **Design Tokens:** Squared Minimalist v2.0 (Obsidian base `#080808`, technical border `#242429`, high-contrast telemetry signals `#FF3333` and `#00E676`)
- **Typography:** `JetBrains Mono` (tabular numbers) + `Space Grotesk` (geometric headers)

---

## Prerequisites

Ensure the following tools and system libraries are installed on your machine:

| Component | Minimum Version | Installation Command (Debian / Ubuntu / Pop!_OS) |
| :--- | :--- | :--- |
| **Rust & Cargo** | `1.75+` (tested on `1.97+`) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Node.js & npm** | Node `v20+` (tested on `v24+`), npm `10+` | `curl -fsSL https://deb.nodesource.com/setup_20.x \| sudo -E bash - && sudo apt install -y nodejs` |
| **PulseAudio / PipeWire** | Runtime library | `sudo apt install -y libpulse0` |
| **SQLite 3** | `3.35+` | `sudo apt install -y sqlite3 libsqlite3-dev` |
| **Build Essentials** | `gcc`, `pkg-config` | `sudo apt install -y build-essential pkg-config` |

---

## Getting Started

### 1. Clone the Repository

```bash
git clone https://github.com/RafiAthallahN/ApexMod.git
cd ApexMod
```

### 2. System Dependencies & Linux Controller Permissions

On Linux, raw gamepad event nodes located in `/dev/input/event*` are owned by `root:input` with `0660` permissions. To allow ApexMod to read controller triggers without needing `root` privileges:

```bash
# Add your current user to the Linux input group
sudo usermod -aG input $USER

# Apply the group membership to your current shell immediately
newgrp input
```

### 3. Build Frontend Production Assets

The Rust binary embeds and serves the compiled Svelte 5 dashboard directly from `web/dist`. Build the web assets before running the daemon:

```bash
cd web
npm install
npm run build
cd ..
```

*Expected output: Vite compiles production assets into `web/dist/` in `< 500ms`.*

### 4. Build & Run the Rust Core Daemon

#### Mode A: Virtual GT3 Telemetry Rig (Testing & Calibration)
To test the visual oscilloscope, audio earcons, and synthetic ACC vehicle metrics without a physical gamepad plugged in:

```bash
cargo run -- --sim
```

#### Mode B: Physical Gamepad & ACC Live Mode
To ingest physical controller hardware triggers alongside live ACC UDP telemetry:

```bash
cargo run
```

### 5. Accessing the Dashboard

Once started, the daemon listens on all interfaces (`0.0.0.0`):

- **Local PC:** Open [http://localhost:3000](http://localhost:3000)
- **Secondary Screen / Phone / Tablet on LAN:** Open `http://<YOUR-PC-IP>:3000` (e.g., `http://192.168.0.113:3000`).

---

## Connecting with Assetto Corsa Competizione (ACC)

ApexMod receives live vehicle telemetry from ACC using **ACC's Native UDP Broadcasting Protocol**. This requires zero game modifications and runs completely out-of-process.

### Configuring ACC UDP Broadcasting

In your ACC Steam directory or Wine/Proton prefix, locate `broadcasting.json`:

```text
# Standard Proton Prefix Path:
~/.steam/steam/steamapps/compatdata/805550/pfx/drive_c/users/steamuser/Documents/Assetto Corsa Competizione/Config/broadcasting.json
```

Edit the file to enable UDP telemetry broadcast on port `9000`:

```json
{
  "updListenerPort": 9000,
  "connectionPassword": "asd",
  "commandPassword": "",
  "dumpIntervalMs": 16,
  "dumpTelemetry": true
}
```

*Note: Ensure UDP port `9000` is allowed in your firewall if ACC is running on a separate host: `sudo ufw allow 9000/udp`.*

### Parallel Hardware Ingestion Flow

```
┌────────────────────────────────────────────────────────┐
│                   WINE / PROTON PREFIX                 │
│                                                        │
│   Assetto Corsa Competizione (ACC)                     │
│   └── UDP Broadcasting Service (broadcasting.json)     │
└─────────────────────────┬──────────────────────────────┘
                          │ 127.0.0.1:9000 (UDP)
                          ▼
┌────────────────────────────────────────────────────────┐
│                   NATIVE LINUX DAEMON                  │
│                                                        │
│  ┌───────────────────────┐   ┌──────────────────────┐  │
│  │ Gamepad evdev Reader  │   │ ACC UDP Ingestion    │  │
│  │ (250Hz trigger inputs)│   │ (Speed, Gear, ABS/TC)│  │
│  └───────────┬───────────┘   └──────────┬───────────┘  │
│              │                          │              │
│              └───────────┬──────────────┘              │
│                          ▼                             │
│             ┌─────────────────────────┐                │
│             │ Telemetry Packet Merger │                │
│             │ (Time-synced frame)     │                │
│             └────────────┬────────────┘                │
│                          │                             │
│         ┌────────────────┴────────────────┐            │
│         ▼                                 ▼            │
│  ┌──────────────┐                 ┌───────────────┐    │
│  │ Audio Engine │                 │ Async Storage │    │
│  │ (Instant cue)│                 │ Ring Buffer   │    │
│  └──────────────┘                 └───────┬───────┘    │
└───────────────────────────────────────────┼────────────┘
                                            │ Batch insert (500ms)
                                            ▼
                                 ┌─────────────────────┐
                                 │ SQLite Local DB     │
                                 │ (apexmod.db - WAL)  │
                                 └─────────────────────┘
```

Both your sim racing title and ApexMod read from `/dev/input/event*` simultaneously without input latency or exclusive device locks.

---

## Architecture & Deep System Design

### Directory Structure

```
apex-mod/
├── Cargo.toml               # Rust workspace manifest & dependencies
├── src/
│   ├── main.rs              # Daemon entry point, 250Hz core loop & server joins
│   ├── config.rs            # Calibration profiles, deadzones, thresholds & EMA alpha
│   ├── audio/
│   │   └── mod.rs           # Procedural stereo FM synthesis engine (libpulse-simple FFI)
│   ├── input/
│   │   ├── mod.rs           # InputManager orchestrating hardware & simulation sources
│   │   ├── acc_udp.rs       # Non-blocking ACC UDP telemetry receiver (port 9000)
│   │   ├── linux_evdev.rs   # Non-blocking Linux /dev/input/event* trigger reader
│   │   └── simulator.rs     # Realistic Monza GT3 cornering telemetry generator
│   ├── physics/
│   │   └── mod.rs           # EMA filter, 1st/2nd derivative evaluator & Modulation Index
│   ├── server/
│   │   └── mod.rs           # Axum HTTP server, REST endpoints & WebSocket broadcasters
│   └── storage/
│       └── mod.rs           # SQLite WAL persistence with async 10k-sample batch writer
└── web/                     # Svelte 5 + Vite telemetry frontend
    ├── package.json
    ├── vite.config.ts
    ├── index.html
    └── src/
        ├── app.css          # Design System v2.0 tokens, typography & spatial grid
        ├── App.svelte       # Cockpit console, status banner & 12-column layout
        └── lib/
            ├── types.ts     # TelemetryFrame, Session & CalibrationProfile interfaces
            └── components/
                ├── TriggerModule.svelte        # 20-segment LED bar, readout & diagnostic box
                ├── DualTraceOscilloscope.svelte# 120 FPS HTML5 Canvas 2D rolling dual trace
                └── CarStateTiles.svelte        # Speed, Gear, Modulation Score & Lap tiles
```

### Multi-Threaded Execution Model

ApexMod isolates hard real-time mathematical operations from network broadcasts and disk I/O using dedicated threads:

1. **Thread 1 (`apexmod-250hz-core`):** Deterministic $4.0\text{ ms}$ fixed-step loop. Polls `evdev` triggers, polls UDP socket buffer, calculates EMA smoothing and derivatives, evaluates dump/snap events, and drives the audio thread.
2. **Thread 2 (`apexmod-earcon-synth`):** Dedicated low-latency audio synthesis loop streaming $4.0\text{ ms}$ chunks ($192$ samples at $48\text{ kHz}$) to PulseAudio / PipeWire.
3. **Thread 3 (`apexmod-db-writer`):** Dedicated SQLite batch writer. Pulls frames from a bounded $10,000$-sample queue and flushes bulk `INSERT` transactions every $500\text{ ms}$ or $300$ samples.
4. **Async Runtime (`tokio`):** Manages the Axum HTTP server on port `3000`, the dedicated WebSocket broadcast stream on port `8765`, and REST API requests.

### Data Pipeline & Mathematical Formulations

```
Raw evdev (0-255) ──> Deadzone Normalization ──> 1st-order EMA ──> Derivative & Jerk Evaluator
                                                                          │
                                     ┌────────────────────────────────────┴────────────────┐
                                     ▼                                                     ▼
                              Event Detection                                     Modulation Index
                        (BrakeDump / ThrottleSnap)                              (Smoothness 0-100%)
```

1. **Deadzone Normalization:**
   Maps raw displacement across inner and outer deadzone thresholds:
   $$\text{Norm}(x) = \operatorname{clamp}\left(\frac{\operatorname{clamp}(x, dz_{\text{in}}, dz_{\text{out}}) - dz_{\text{in}}}{dz_{\text{out}} - dz_{\text{in}}}, 0.0, 1.0\right)$$

2. **First-Order Exponential Moving Average (EMA):**
   Filters hardware sensor noise from cheap trigger potentiometers while preserving response sharpness:
   $$y_t = \alpha \cdot x_t + (1 - \alpha) \cdot y_{t-1} \quad (\text{default } \alpha = 0.35)$$

3. **Derivative & Jerk Calculations:**
   $$\text{Velocity } v_t = \frac{y_t - y_{t-1}}{\Delta t}, \quad \text{Jerk } j_t = \frac{v_t - v_{t-1}}{\Delta t}$$

4. **Event Detection Logic:**
   - **`BrakeDump`:** Triggered when the driver held $> 70\%$ brake within the past $350\text{ ms}$ and executes a release rate $v_{\text{brk}} < -V_{\text{dump\_threshold}}$ (default $-150\%/\text{s}$).
   - **`ThrottleSnap`:** Triggered when transitioning from low throttle ($< 20\%$) and spikes $v_{\text{thr}} > +V_{\text{snap\_threshold}}$ (default $+180\%/\text{s}$).

5. **Modulation Index ($0\text{–}100\%$):**
   Calculates driver input smoothness continuously from jerk during active transitions:
   $$\text{Score}_t = \text{Score}_{t-1} \cdot (1 - \beta) + \left(100.0 - \frac{|j_{\text{brk}}| \cdot 0.6 + |j_{\text{thr}}| \cdot 0.4}{60.0}\right) \cdot \beta$$

### Audio Synthesis Engine (Spatial Earcons)

- **Acoustic Latency:** $< 10\text{ ms}$ end-to-end buffer latency ($192$ samples at $48\text{ kHz}$).
- **Stereo Channel Isolation:** Left channel is reserved strictly for Brake decay & dump chirps; Right channel is reserved strictly for Throttle ramp & snap clicks.
- **Dynamic FFI:** Binds to `libpulse-simple.so.0` at runtime using `libloading`, eliminating compile-time `-dev` header requirements.

### Network & Transport Architecture

ApexMod exposes dual WebSocket transports to support both local desktop usage and remote mobile/tablet mounts:

| Endpoint | Protocol | Purpose | Frequency |
| :--- | :--- | :--- | :--- |
| `ws://0.0.0.0:8765` | Standalone WebSocket | Dedicated low-overhead telemetry broadcast (`tokio-tungstenite`) | $60\text{ Hz}$ |
| `http://0.0.0.0:3000/ws` | Proxied WebSocket | Standard Axum WebSocket upgrade for LAN browsers and reverse proxies | $60\text{ Hz}$ |
| `http://0.0.0.0:3000` | HTTP / Static | Serves Svelte 5 dashboard assets (`web/dist/`) and REST endpoints | On Demand |
| `udp://127.0.0.1:9000` | UDP Socket | Ingests ACC car state and broadcast telemetry | $\approx 60\text{ Hz}$ |

#### Telemetry Frame Payload (JSON)
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
  "device_name": "Microsoft Xbox Wireless Controller",
  "speed_kmh": 142.6,
  "gear": 3,
  "abs_active": true,
  "tc_active": false,
  "lap_index": 4,
  "lap_time_ms": 78420,
  "track_id": "monza",
  "car_model": "porsche_992_gt3_r",
  "udp_connected": true
}
```

### Database Schema (SQLite WAL & Async Ring Buffer)

Stored locally in `apexmod.db` with `PRAGMA journal_mode = WAL;` and `PRAGMA synchronous = NORMAL;`:

```sql
-- Stint Session Metadata
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL,
    car_model TEXT NOT NULL,
    controller_name TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    total_laps INTEGER DEFAULT 0,
    avg_modulation_score REAL DEFAULT 0.0
);

-- Lap Summary Records
CREATE TABLE IF NOT EXISTS laps (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    lap_number INTEGER NOT NULL,
    lap_time_ms INTEGER,
    is_valid BOOLEAN DEFAULT 1,
    abs_events_count INTEGER DEFAULT 0,
    tc_events_count INTEGER DEFAULT 0,
    brake_dump_count INTEGER DEFAULT 0,
    throttle_snap_count INTEGER DEFAULT 0,
    modulation_score REAL DEFAULT 0.0,
    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- Continuous Timeseries Telemetry Frames
CREATE TABLE IF NOT EXISTS telemetry_frames (
    lap_id TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    trigger_brake REAL NOT NULL,
    trigger_throttle REAL NOT NULL,
    d_brake_dt REAL NOT NULL,
    d_throttle_dt REAL NOT NULL,
    speed_kmh REAL NOT NULL,
    gear INTEGER NOT NULL,
    abs_active INTEGER NOT NULL,
    tc_active INTEGER NOT NULL,
    PRIMARY KEY (lap_id, timestamp_ms),
    FOREIGN KEY(lap_id) REFERENCES laps(id) ON DELETE CASCADE
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_telemetry_lap ON telemetry_frames(lap_id, timestamp_ms);
```

### REST API Reference

| Method | Route | Description |
| :--- | :--- | :--- |
| `GET` | `/api/status` | Returns active calibration profile and simulation state |
| `GET` | `/api/sessions` | Lists the 20 most recent stint sessions and average scores |
| `POST` | `/api/config` | Updates calibration profile (deadzones, EMA alpha, dump/snap thresholds) |
| `POST` | `/api/simulate` | Toggles the virtual GT3 simulation rig (`{"enabled": true}`) |

---

## Design System v2.0 (Squared Minimalist)

ApexMod implements an unapologetic, instrument-grade technical console inspired by physical motorsport telemetry hardware (MoTeC, Bosch DDU):

- **Strict Orthogonality:** `border-radius: 0px !important;` on all cards, buttons, badges, and containers.
- **Structural Elevation:** Depth is communicated through `1px solid #242429` borders and stepped fills (`#080808` base, `#101012` surface, `#17171A` elevated) rather than drop shadows.
- **Peripheral Inversion:** Hardware diagnostic boxes under each trigger invert immediately on violation states (e.g., solid `#2B0B0B` fill with `#FF3333` border on brake dump).

```
┌────────────────────────────────────────────────────────────────────────┐
│ [APEXMOD] WS: ONLINE  UDP :9000 LINKED   [ACC GT3] [MONZA] [PORSCHE]   │
├───────────────────┬────────────────────────────────┬───────────────────┤
│ LEFT TRIGGER (LT) │  DUAL-TRACE OSCILLOSCOPE (120F)│ RIGHT TRIGGER(RT) │
│                   │                                │                   │
│ [20-SEG LED BAR]  │  ============================  │ [20-SEG LED BAR]  │
│ 84%               │  ----------------------------  │ 100%              │
│ d/dt: -182%/S     │                                │ d/dt: +42%/S      │
│                   ├───────┬───────┬────────┬───────┤                   │
│                   │ SPEED │ GEAR  │ SCORE  │  LAP  │                   │
│                   │142KM/H│   3   │ 88.5%  │  #04  │                   │
├───────────────────┴───────┴───────┴────────┴───────┴───────────────────┤
│ DIAGNOSTIC: BRAKE DUMP DETECTED (-182%/S) // ABS LOCK ACTIVE           │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Configuration & Environment Variables

### Command-Line Arguments

| Flag | Description | Default |
| :--- | :--- | :--- |
| `--sim`, `--simulate` | Starts daemon in virtual GT3 Corner Simulator mode | `false` |

### Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `APEXMOD_SIM` | Enable simulator mode via environment (`1` or `true`) | `0` |
| `RUST_LOG` | Logging verbosity filter (`apex_mod=info`, `debug`, `trace`) | `apex_mod=info` |

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

### Frontend Dashboard (`npm` inside `web/`)

| Command | Description |
| :--- | :--- |
| `npm run dev` | Start Vite dev server on `http://localhost:5173` with HMR and backend proxy |
| `npm run build` | Compile Svelte 5 application into production static assets (`web/dist/`) |
| `npx tsc --noEmit` | Validate TypeScript types |

---

## Testing & Quality Verification

ApexMod includes deterministic unit and integration test suites:

```bash
# Run backend test suite
cargo test
```

### Key Automated Tests

1. `physics::tests::detects_brake_dump_and_throttle_snap`: Verifies that sudden transitions trigger corresponding fault events and threshold penalties.
2. `input::simulator::tests::clean_lap_vs_fault_lap_simulation_integrates_with_ema_physics`: Verifies that a clean simulated lap generates **zero** false-positive fault events with EMA filtering enabled, and that fault laps reliably trigger both `BrakeDump` and `ThrottleSnap`.
3. `web` Typecheck: `cd web && npx tsc --noEmit` guarantees all frontend contracts and component props match backend types.

---

## Production & Background Daemon Setup (systemd)

To run ApexMod as a persistent Linux user service that starts automatically upon system login:

1. Create a systemd user unit file:
   ```bash
   mkdir -p ~/.config/systemd/user
   nano ~/.config/systemd/user/apexmod.service
   ```

2. Add the following unit configuration:
   ```ini
   [Unit]
   Description=ApexMod Sim Racing Telemetry Daemon
   After=sound.target network.target

   [Service]
   Type=simple
   WorkingDirectory=%h/Documents/rnd/tauri/apex-mod
   ExecStart=%h/Documents/rnd/tauri/apex-mod/target/release/apex-mod
   Restart=always
   RestartSec=3

   [Install]
   WantedBy=default.target
   ```

3. Build the release binary and enable the service:
   ```bash
   cargo build --release
   systemctl --user daemon-reload
   systemctl --user enable --now apexmod.service
   systemctl --user status apexmod.service
   ```

---

## Troubleshooting

### 1. Mobile Phone or Tablet Cannot Connect ("Site can't be reached")

- **Explicitly use `http://` (not `https://`):**
  Mobile browsers (Safari on iOS, Chrome on Android) often auto-complete IP addresses with `https://`. Because ApexMod runs a local unencrypted dashboard, an HTTPS request will fail. Use:
  ```text
  http://192.168.0.113:3000
  ```
- **Allow Port in Linux Firewall (`ufw`):**
  If `ufw` is active on Linux, incoming connections from other devices are blocked by default:
  ```bash
  sudo ufw allow 3000/tcp
  sudo ufw allow 8765/tcp
  sudo ufw allow 9000/udp
  ```
- **Wi-Fi Router AP Isolation (Client Isolation):**
  If you cannot ping your phone from your PC (`ping <PHONE-IP>`), your Wi-Fi router has Client Isolation turned on.
  - **Quick Tunnel Bypass:** Run an instant tunnel on your PC:
    ```bash
    ssh -R 80:localhost:3000 nokey@localhost.run
    ```
    Open the generated URL on your phone.
  - **Zero-Latency Sim Rig Solution:** Connect your phone via USB cable and enable **USB Tethering** in phone settings.

### 2. Gamepad Shows "Uncalibrated / PermissionDenied"

If the dashboard displays `Uncalibrated (PermissionDenied on /dev/input/event*)`:
Your user account does not have read permissions for Linux input device nodes.
```bash
sudo usermod -aG input $USER
newgrp input
```

### 3. ACC UDP Telemetry Shows "STBY" Instead of "LINKED"

- Verify that `broadcasting.json` has `"updListenerPort": 9000` and `"dumpTelemetry": true`.
- Check if port `9000` is already in use by another tool:
  ```bash
  ss -ulpn | grep 9000
  ```
- If running ACC on Windows and streaming to a Linux PC over LAN, verify UDP port `9000` is open:
  ```bash
  sudo ufw allow 9000/udp
  ```

### 4. Audio Cues Are Silent or Missing

- Check PulseAudio / PipeWire service status:
  ```bash
  pactl info
  ```
- Verify `libpulse-simple.so.0` is present:
  ```bash
  find /usr/lib* -name "libpulse-simple.so*"
  ```
- Unmute in the console: Click the **`EARCONS: MUTE`** button in the top navigation bar to toggle active sound feedback.

### 5. Frontend Vite Build Fails

Ensure Node.js is version 20 or higher:
```bash
node -v
cd web
rm -rf node_modules package-lock.json
npm install
npm run build
```

---

## Contributing

Pull requests and issues are welcome! Please ensure:
1. All unit tests pass: `cargo test`.
2. Clippy linting is clean: `cargo clippy --all-targets -- -D warnings`.
3. Svelte 5 frontend builds cleanly: `cd web && npx tsc --noEmit && npm run build`.

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
