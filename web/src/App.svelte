<script lang="ts">
  import { onMount } from 'svelte';
  import type { CalibrationProfile, TelemetryFrame } from './lib/types';
  import TriggerModule from './lib/components/TriggerModule.svelte';
  import DualTraceOscilloscope, {
    type DualSamplePoint
  } from './lib/components/DualTraceOscilloscope.svelte';
  import CarStateTiles from './lib/components/CarStateTiles.svelte';

  let frame = $state<TelemetryFrame>({
    t: 0,
    brk: 0,
    thr: 0,
    d_brk: 0,
    d_thr: 0,
    event: null,
    score: 100.0,
    connected: false,
    device_name: 'Uncalibrated / Disconnected',
    speed_kmh: 0,
    gear: 0,
    abs_active: false,
    tc_active: false,
    lap_index: 1,
    lap_time_ms: 0,
    track_id: 'monza',
    car_model: 'porsche_992_gt3_r',
    udp_connected: false
  });

  let profile = $state<CalibrationProfile>({
    id: 'default',
    device_name: 'Generic Controller',
    brake_deadzone: { inner: 0.02, outer: 0.98 },
    throttle_deadzone: { inner: 0.02, outer: 0.98 },
    ema_alpha: 0.35,
    dump_velocity_threshold: 1.5,
    snap_velocity_threshold: 1.8,
    audio_muted: false,
    audio_volume: 0.25
  });

  let simulationEnabled = $state(false);
  let wsConnected = $state(false);
  let showCalibrationDrawer = $state(false);

  let dualHistory = $state<DualSamplePoint[]>([]);

  let brakeViolation = $state(false);
  let brakeViolationVel = $state(0);
  let throttleViolation = $state(false);
  let throttleViolationVel = $state(0);

  let brakeViolationTimer: ReturnType<typeof setTimeout> | null = null;
  let throttleViolationTimer: ReturnType<typeof setTimeout> | null = null;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  let activeSocket: WebSocket | null = null;
  let useProxyWsFallback = false;

  async function fetchInitialStatus() {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        profile = data.profile;
        simulationEnabled = data.simulation_enabled;
      }
    } catch {
      // Daemon might be starting up
    }
  }

  async function toggleMute() {
    profile.audio_muted = !profile.audio_muted;
    await saveProfile();
  }

  async function saveProfile() {
    try {
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(profile)
      });
    } catch {
      // Ignore network error during offline preview
    }
  }

  async function activateControllerOrSim() {
    simulationEnabled = !simulationEnabled;
    try {
      await fetch('/api/simulate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: simulationEnabled })
      });
    } catch {
      // Ignore network error
    }
  }

  function connectWebSocket() {
    if (activeSocket) {
      activeSocket.onclose = null;
      activeSocket.close();
      activeSocket = null;
    }

    const isLocalhost =
      window.location.hostname === '127.0.0.1' || window.location.hostname === 'localhost';
    const proxyWsUrl = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/ws`;
    const wsUrl = isLocalhost && !useProxyWsFallback ? 'ws://127.0.0.1:8765' : proxyWsUrl;

    const ws = new WebSocket(wsUrl);
    activeSocket = ws;

    ws.onopen = () => {
      wsConnected = true;
    };

    ws.onmessage = (ev) => {
      try {
        const incoming: TelemetryFrame = JSON.parse(ev.data);
        frame = incoming;
        const now = performance.now();
        const cutoff = now - 3200;

        dualHistory.push({ t: now, brk: incoming.brk, thr: incoming.thr });
        if (dualHistory.length > 240) {
          dualHistory = dualHistory.filter((p) => p.t >= cutoff);
        }

        if (incoming.event === 'BRAKE_DUMP') {
          brakeViolation = true;
          brakeViolationVel = incoming.d_brk * 100;
          if (brakeViolationTimer) clearTimeout(brakeViolationTimer);
          brakeViolationTimer = setTimeout(() => {
            brakeViolation = false;
          }, 450);
        } else if (incoming.event === 'THROTTLE_SNAP') {
          throttleViolation = true;
          throttleViolationVel = incoming.d_thr * 100;
          if (throttleViolationTimer) clearTimeout(throttleViolationTimer);
          throttleViolationTimer = setTimeout(() => {
            throttleViolation = false;
          }, 450);
        }
      } catch {
        // Ignore malformed packet
      }
    };

    ws.onclose = () => {
      wsConnected = false;
      useProxyWsFallback = !useProxyWsFallback;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      reconnectTimer = setTimeout(connectWebSocket, 1200);
    };
  }

  onMount(() => {
    fetchInitialStatus();
    connectWebSocket();

    return () => {
      if (reconnectTimer) clearTimeout(reconnectTimer);
      if (brakeViolationTimer) clearTimeout(brakeViolationTimer);
      if (throttleViolationTimer) clearTimeout(throttleViolationTimer);
      if (activeSocket) {
        activeSocket.onclose = null;
        activeSocket.close();
      }
    };
  });
</script>

<main class="console-root">
  <!-- 5.1 System Status Bar (Header) -->
  <header class="status-banner">
    <div class="status-left">
      <span class="brand-badge">APEXMOD</span>
      <span class="pipeline-item micro-label" class:online={wsConnected}>
        WS: {wsConnected ? 'ONLINE' : 'SYNCING'}
      </span>
      <span class="pipeline-item micro-label" class:online={frame.udp_connected}>
        UDP :9000 {frame.udp_connected ? 'LINKED' : 'STBY'}
      </span>
      <span class="pipeline-item micro-label">250HZ POLL // SQLITE WAL</span>
    </div>

    <div class="status-right">
      <span class="meta-badge micro-label">ACC GT3</span>
      <span class="meta-badge micro-label">{(frame.track_id || 'MONZA').toUpperCase()}</span>
      <span class="meta-badge micro-label">
        {(frame.car_model || 'PORSCHE_992_GT3_R').replace(/_/g, ' ').toUpperCase()}
      </span>

      <button
        class="ctrl-btn micro-label"
        class:btn-active={!profile.audio_muted}
        onclick={toggleMute}
      >
        EARCONS: {profile.audio_muted ? 'MUTE' : 'ON'}
      </button>
      <button
        class="ctrl-btn micro-label"
        class:btn-active={simulationEnabled}
        onclick={activateControllerOrSim}
      >
        SIM RIG: {simulationEnabled ? 'ON' : 'OFF'}
      </button>
      <button
        class="ctrl-btn micro-label"
        class:btn-active={showCalibrationDrawer}
        onclick={() => (showCalibrationDrawer = !showCalibrationDrawer)}
      >
        CALIBRATE
      </button>
    </div>
  </header>

  {#if showCalibrationDrawer}
    <section class="calibration-panel">
      <div class="cal-box">
        <label class="micro-label" for="ema">EMA FILTER (α): {profile.ema_alpha.toFixed(2)}</label>
        <input
          id="ema"
          type="range"
          min="0.05"
          max="1.0"
          step="0.05"
          bind:value={profile.ema_alpha}
          onchange={saveProfile}
        />
      </div>
      <div class="cal-box">
        <label class="micro-label" for="dump">
          BRAKE DUMP THRESHOLD: -{Math.round(profile.dump_velocity_threshold * 100)}%/S
        </label>
        <input
          id="dump"
          type="range"
          min="0.5"
          max="3.0"
          step="0.1"
          bind:value={profile.dump_velocity_threshold}
          onchange={saveProfile}
        />
      </div>
      <div class="cal-box">
        <label class="micro-label" for="snap">
          THROTTLE SNAP THRESHOLD: +{Math.round(profile.snap_velocity_threshold * 100)}%/S
        </label>
        <input
          id="snap"
          type="range"
          min="0.5"
          max="3.0"
          step="0.1"
          bind:value={profile.snap_velocity_threshold}
          onchange={saveProfile}
        />
      </div>
    </section>
  {/if}

  <!-- 4.2 Responsive 12-Column Cockpit Grid -->
  <div class="cockpit-grid" class:dimmed={!frame.connected}>
    <!-- Left Trigger (3 cols on Desktop) -->
    <div class="col-left">
      <TriggerModule
        channel="brake"
        tag="LT"
        label="BRAKE CHANNEL"
        liveValue={frame.brk}
        liveVelocity={frame.d_brk}
        assistActive={frame.abs_active}
        assistLabel="ABS"
        violationActive={brakeViolation}
        violationVelocity={brakeViolationVel}
      />
    </div>

    <!-- Center Column: Oscilloscope & ACC Vehicle Telemetry (6 cols on Desktop, pinned to top on Mobile) -->
    <div class="col-center">
      <DualTraceOscilloscope
        history={dualHistory}
        liveBrake={frame.brk}
        liveThrottle={frame.thr}
      />

      <CarStateTiles
        speedKmh={frame.speed_kmh}
        gear={frame.gear}
        modulationScore={frame.score}
        lapIndex={frame.lap_index}
        lapTimeMs={frame.lap_time_ms}
      />
    </div>

    <!-- Right Trigger (3 cols on Desktop) -->
    <div class="col-right">
      <TriggerModule
        channel="throttle"
        tag="RT"
        label="THROTTLE CHANNEL"
        liveValue={frame.thr}
        liveVelocity={frame.d_thr}
        assistActive={frame.tc_active}
        assistLabel="TC"
        violationActive={throttleViolation}
        violationVelocity={throttleViolationVel}
      />
    </div>
  </div>

  <!-- Hardware Status Footer Bar -->
  <footer class="footer-bar">
    <span class="micro-label">CONTROLLER: {frame.device_name || 'DISCONNECTED'}</span>
    {#if !frame.connected}
      <button class="detect-btn micro-label" onclick={activateControllerOrSim}>
        [ DETECT CONTROLLER OR START VIRTUAL GT3 RIG ]
      </button>
    {:else}
      <span class="micro-label">ARCHIVE: SESSIONS.DB (WAL) // 500MS BATCH FLUSH</span>
    {/if}
  </footer>
</main>

<style>
  .console-root {
    min-height: 100vh;
    padding: var(--p-3);
    display: flex;
    flex-direction: column;
    gap: var(--p-3);
    max-width: 1600px;
    margin: 0 auto;
  }

  /* 5.1 System Status Bar (Header) */
  .status-banner {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    padding: var(--p-2) var(--p-3);
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--p-2);
  }

  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--p-2);
  }

  /* Solid white badge (#EDEDED background with #080808 text) */
  .brand-badge {
    background: var(--brand-accent);
    color: var(--brand-black);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 900;
    padding: 4px 8px;
    letter-spacing: 0.08em;
  }

  .pipeline-item {
    padding: 4px 8px;
    border: 1px solid var(--brand-border);
    background: var(--brand-black);
  }

  .pipeline-item.online {
    color: var(--brand-throttle);
    border-color: var(--brand-borderLight);
  }

  .meta-badge {
    padding: 4px 8px;
    border: 1px solid var(--brand-border);
    background: var(--brand-elevated);
    color: var(--brand-accent);
  }

  .ctrl-btn {
    padding: 4px 10px;
    border: 1px solid var(--brand-borderLight);
    background: var(--brand-elevated);
    color: var(--brand-muted);
    cursor: pointer;
  }

  .ctrl-btn:hover {
    border-color: var(--brand-accent);
    color: var(--brand-accent);
  }

  .ctrl-btn.btn-active {
    background: var(--brand-accent);
    color: var(--brand-black);
    border-color: var(--brand-accent);
  }

  .calibration-panel {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    padding: var(--p-3);
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: var(--p-3);
  }

  .cal-box {
    background: var(--brand-black);
    border: 1px solid var(--brand-border);
    padding: var(--p-2);
    display: flex;
    flex-direction: column;
    gap: var(--p-2);
  }

  /* Desktop (> 1024px): 12-column layout: Left Trigger (3 cols), Center (6 cols), Right Trigger (3 cols) */
  .cockpit-grid {
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    gap: var(--p-3);
    align-items: stretch;
  }

  .cockpit-grid.dimmed {
    opacity: 0.55;
  }

  .col-left {
    grid-column: span 3;
    display: flex;
    flex-direction: column;
  }

  .col-center {
    grid-column: span 6;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: var(--p-3);
  }

  .col-right {
    grid-column: span 3;
    display: flex;
    flex-direction: column;
  }

  .col-left > :global(*),
  .col-right > :global(*) {
    flex: 1;
  }

  .footer-bar {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    padding: var(--p-2) var(--p-3);
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--p-2);
  }

  .detect-btn {
    background: var(--brand-throttle);
    color: var(--brand-black);
    border: 1px solid var(--brand-throttle);
    padding: 4px 10px;
    cursor: pointer;
  }

  /* Tablet (768px - 1024px): Scope top (12 cols), Left & Right Triggers side-by-side below (6 cols each) */
  @media (max-width: 1024px) {
    .col-center {
      grid-column: span 12;
      order: -1;
    }

    .col-left,
    .col-right {
      grid-column: span 6;
    }
  }

  /* Mobile / Phone Mount (< 768px): Scope pinned to top; Left/Right Triggers stacked below */
  @media (max-width: 767px) {
    .col-center,
    .col-left,
    .col-right {
      grid-column: span 12;
    }
  }
</style>
