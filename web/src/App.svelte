<script lang="ts">
  import { onMount } from 'svelte';
  import type { CalibrationProfile, ModulationEventType, TelemetryFrame } from './lib/types';
  import EventAlertBanner from './lib/components/EventAlertBanner.svelte';
  import LiveInputRibbon from './lib/components/LiveInputRibbon.svelte';
  import RadialJerkGauge from './lib/components/RadialJerkGauge.svelte';

  interface SamplePoint {
    t: number;
    val: number;
  }

  let frame = $state<TelemetryFrame>({
    t: 0,
    brk: 0,
    thr: 0,
    d_brk: 0,
    d_thr: 0,
    event: null,
    score: 100.0,
    connected: false,
    device_name: 'Uncalibrated / Disconnected'
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

  let brakeHistory = $state<SamplePoint[]>([]);
  let throttleHistory = $state<SamplePoint[]>([]);

  let activeBannerEvent = $state<ModulationEventType>(null);
  let activeBannerVelocity = $state(0);
  let brakeViolation = $state(false);
  let throttleViolation = $state(false);

  let bannerTimer: ReturnType<typeof setTimeout> | null = null;
  let violationTimer: ReturnType<typeof setTimeout> | null = null;
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

        brakeHistory.push({ t: now, val: incoming.brk });
        throttleHistory.push({ t: now, val: incoming.thr });

        if (brakeHistory.length > 240) {
          brakeHistory = brakeHistory.filter((p) => p.t >= cutoff);
        }
        if (throttleHistory.length > 240) {
          throttleHistory = throttleHistory.filter((p) => p.t >= cutoff);
        }

        if (incoming.event) {
          activeBannerEvent = incoming.event;
          activeBannerVelocity =
            incoming.event === 'BRAKE_DUMP' ? incoming.d_brk * 100 : incoming.d_thr * 100;

          if (bannerTimer) clearTimeout(bannerTimer);
          bannerTimer = setTimeout(() => {
            activeBannerEvent = null;
          }, 400);

          if (incoming.event === 'BRAKE_DUMP') {
            brakeViolation = true;
          } else if (incoming.event === 'THROTTLE_SNAP') {
            throttleViolation = true;
          }

          if (violationTimer) clearTimeout(violationTimer);
          violationTimer = setTimeout(() => {
            brakeViolation = false;
            throttleViolation = false;
          }, 200);
        }
      } catch {
        // Ignore malformed packet
      }
    };

    ws.onclose = () => {
      wsConnected = false;
      // Alternate between direct port 8765 and HTTP /ws endpoint on reconnect
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
      if (bannerTimer) clearTimeout(bannerTimer);
      if (violationTimer) clearTimeout(violationTimer);
      if (activeSocket) {
        activeSocket.onclose = null;
        activeSocket.close();
      }
    };
  });
</script>

<main class="cockpit-shell">
  <!-- Top Bar -->
  <header class="topbar">
    <div class="brand">
      <span class="brand-mark">APEXMOD</span>
      <span class="caption-unit">
        {frame.device_name || 'Uncalibrated'} · {wsConnected ? '60Hz WS LIVE' : 'RECONNECTING'}
      </span>
    </div>

    <!-- Component C: Event Alert Banner / Earcon Visualizer -->
    <EventAlertBanner activeEvent={activeBannerEvent} velocityPctPerSec={activeBannerVelocity} />

    <!-- Zero In-Game Friction Controls (<= 3 clicks, Design.md §1.2) -->
    <div class="quick-actions">
      <button class="btn-action" onclick={toggleMute}>
        {profile.audio_muted ? 'EARCONS: MUTED' : 'EARCONS: ACTIVE'}
      </button>
      <button class="btn-action" onclick={activateControllerOrSim}>
        {simulationEnabled ? 'SIM RIG: ON' : 'SIM RIG: OFF'}
      </button>
      <button
        class="btn-action"
        onclick={() => (showCalibrationDrawer = !showCalibrationDrawer)}
      >
        CALIBRATE
      </button>
    </div>
  </header>

  {#if showCalibrationDrawer}
    <section class="calibration-bar">
      <div class="cal-item">
        <label class="caption-unit" for="ema">EMA FILTER (α): {profile.ema_alpha.toFixed(2)}</label>
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
      <div class="cal-item">
        <label class="caption-unit" for="dump">
          BRAKE DUMP LIMIT: -{Math.round(profile.dump_velocity_threshold * 100)}%/s
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
      <div class="cal-item">
        <label class="caption-unit" for="snap">
          THROTTLE SNAP LIMIT: +{Math.round(profile.snap_velocity_threshold * 100)}%/s
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

  <!-- Main Telemetry Grid (Compact stacked vs Expanded 3-column horizontal ribbon) -->
  <div class="telemetry-workspace" class:dimmed={!frame.connected}>
    <!-- Left Channel: Brake Ribbon (Component A) -->
    <LiveInputRibbon
      channel="brake"
      label="BRAKE (LT)"
      liveValue={frame.brk}
      liveVelocity={frame.d_brk}
      history={brakeHistory}
      violationActive={brakeViolation}
    />

    <!-- Center HUD: Radial Jerk & Velocity Gauge (Component B) -->
    <RadialJerkGauge
      brakeVelocity={frame.d_brk}
      throttleVelocity={frame.d_thr}
      score={frame.score}
      dumpThreshold={profile.dump_velocity_threshold}
      snapThreshold={profile.snap_velocity_threshold}
    />

    <!-- Right Channel: Throttle Ribbon (Component A) -->
    <LiveInputRibbon
      channel="throttle"
      label="THROTTLE (RT)"
      liveValue={frame.thr}
      liveVelocity={frame.d_thr}
      history={throttleHistory}
      violationActive={throttleViolation}
    />
  </div>

  <!-- Uncalibrated / Disconnected Overlay (Design.md §6) -->
  {#if !frame.connected}
    <div class="disconnected-overlay">
      <div class="disconnected-card">
        <p class="body-regular">No hardware evdev trigger activity detected.</p>
        <button class="btn-detect" onclick={activateControllerOrSim}>
          Detect Controller (Move Triggers) / Start Virtual GT3 Rig
        </button>
      </div>
    </div>
  {/if}
</main>

<style>
  .cockpit-shell {
    min-height: 100vh;
    background: var(--bg-base);
    padding: var(--space-md);
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
    position: relative;
  }

  .topbar {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: var(--space-sm);
    padding-bottom: var(--space-sm);
    border-bottom: 1px solid var(--border-subtle);
  }

  .brand {
    display: flex;
    flex-direction: column;
    gap: var(--space-xxs);
  }

  .brand-mark {
    font-family: var(--font-mono);
    font-size: 16px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--text-primary);
  }

  .quick-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-xs);
  }

  .btn-action {
    background: var(--bg-surface-elevated);
    color: var(--text-primary);
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    padding: var(--space-xs) var(--space-sm);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
  }

  .btn-action:hover {
    border-color: var(--text-secondary);
  }

  .calibration-bar {
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: var(--space-sm);
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: var(--space-md);
  }

  .cal-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
  }

  /* Expanded (> 768px): Dual-channel horizontal ribbon layout with central telemetry HUD */
  .telemetry-workspace {
    display: grid;
    grid-template-columns: 1fr 280px 1fr;
    gap: var(--space-md);
    align-items: stretch;
    transition: opacity 200ms ease;
  }

  /* Uncalibrated / Disconnected state (Design.md §6): 40% opacity */
  .telemetry-workspace.dimmed {
    opacity: 0.4;
  }

  .disconnected-overlay {
    position: fixed;
    inset: 0;
     pointer-events: none;
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 20;
  }

  .disconnected-card {
    pointer-events: auto;
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: var(--space-md);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-sm);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.65);
  }

  .btn-detect {
    background: var(--accent-throttle);
    color: #0D0E11;
    border: none;
    border-radius: 6px;
    padding: 12px var(--space-md);
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 700;
    cursor: pointer;
  }

  /* Compact Breakpoint (360px - 768px, Design.md §4): Single stacked column */
  @media (max-width: 768px) {
    .topbar {
      grid-template-columns: 1fr;
      justify-items: center;
      text-align: center;
    }

    .quick-actions {
      justify-content: center;
    }

    .telemetry-workspace {
      grid-template-columns: 1fr;
    }
  }
</style>
