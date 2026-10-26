<script lang="ts">
  interface Props {
    brakeVelocity: number;
    throttleVelocity: number;
    score: number;
    dumpThreshold: number;
    snapThreshold: number;
  }

  let { brakeVelocity, throttleVelocity, score, dumpThreshold, snapThreshold }: Props = $props();

  // Combine dominant active derivative for the central needle (deflects left on release, right on press)
  let dominantVelocity = $derived(
    Math.abs(brakeVelocity) >= Math.abs(throttleVelocity) ? brakeVelocity : throttleVelocity
  );

  // Clamp needle angle to [-90deg, +90deg] around top center (0.0)
  // Full scale = +/- 2.5 normalized units/sec (+/- 250%/s)
  let needleAngleDeg = $derived(Math.max(-90, Math.min(90, (dominantVelocity / 2.5) * 90)));

  let gaugeColor = $derived.by(() => {
    const absVel = Math.abs(dominantVelocity);
    const limit = dominantVelocity < 0 ? dumpThreshold : snapThreshold;
    if (absVel >= limit) return 'var(--status-critical)';
    if (absVel >= limit * 0.7) return 'var(--status-warning)';
    return 'var(--accent-throttle)';
  });
</script>

<section class="gauge-card">
  <div class="gauge-header">
    <span class="body-regular gauge-title">MODULATION & DERIVATIVE HUD</span>
    <span class="caption-unit">d(input)/dt · 250 Hz</span>
  </div>

  <div class="gauge-dial-wrap">
    <svg viewBox="0 0 220 130" class="dial-svg" aria-label="Radial Jerk and Velocity Gauge">
      <!-- Background Arc (-90 deg to +90 deg) -->
      <path
        d="M 25 110 A 85 85 0 0 1 195 110"
        fill="none"
        stroke="var(--border-subtle)"
        stroke-width="10"
        stroke-linecap="round"
      />

      <!-- Center Zero Tick -->
      <line x1="110" y1="16" x2="110" y2="28" stroke="var(--text-secondary)" stroke-width="2" />

      <!-- Needle deflecting left on release (<0) and right on application (>0) -->
      <g transform="translate(110, 110) rotate({needleAngleDeg})">
        <line
          x1="0"
          y1="0"
          x2="0"
          y2="-76"
          stroke={gaugeColor}
          stroke-width="3.5"
          stroke-linecap="round"
        />
        <circle cx="0" cy="0" r="6" fill={gaugeColor} />
      </g>

      <text x="18" y="126" class="svg-caption">RELEASE (-d/dt)</text>
      <text x="138" y="126" class="svg-caption">PRESS (+d/dt)</text>
    </svg>
  </div>

  <div class="gauge-footer">
    <div class="metric-block">
      <span class="caption-unit">VELOCITY</span>
      <span class="metric-primary" style:color={gaugeColor}>
        {dominantVelocity >= 0 ? '+' : ''}{Math.round(dominantVelocity * 100)}%/s
      </span>
    </div>

    <div class="metric-block align-right">
      <span class="caption-unit">MODULATION INDEX</span>
      <span class="display-large">{score.toFixed(1)}%</span>
    </div>
  </div>
</section>

<style>
  .gauge-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: var(--space-sm);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: var(--space-xs);
  }

  .gauge-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .gauge-title {
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .gauge-dial-wrap {
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .dial-svg {
    width: 100%;
    max-width: 240px;
    height: auto;
  }

  .svg-caption {
    fill: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 500;
  }

  .gauge-footer {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    padding-top: var(--space-xs);
    border-top: 1px solid var(--border-subtle);
  }

  .metric-block {
    display: flex;
    flex-direction: column;
    gap: var(--space-xxs);
  }

  .align-right {
    text-align: right;
  }
</style>
