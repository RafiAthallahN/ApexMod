<script lang="ts">
  interface Props {
    channel: 'brake' | 'throttle';
    label: string;
    tag: string;
    liveValue: number;
    liveVelocity: number;
    assistActive: boolean;
    assistLabel: 'ABS' | 'TC';
    violationActive: boolean;
    violationVelocity: number;
  }

  let {
    channel,
    label,
    tag,
    liveValue,
    liveVelocity,
    assistActive,
    assistLabel,
    violationActive,
    violationVelocity
  }: Props = $props();

  // 20 discrete stacked rectangular LED segments (Design.md v2.0 §5.2)
  const TOTAL_SEGMENTS = 20;
  let activeCount = $derived(Math.round(Math.max(0, Math.min(1, liveValue)) * TOTAL_SEGMENTS));

  let pctDisplay = $derived(Math.round(liveValue * 100));
  let velPctPerSec = $derived(Math.round(liveVelocity * 100));
</script>

<section class="trigger-panel">
  <!-- Module Header -->
  <header class="panel-header">
    <div class="header-left">
      <span class="tag-box" class:tag-brake={channel === 'brake'} class:tag-throttle={channel === 'throttle'}>
        {tag}
      </span>
      <span class="header-section">{label}</span>
    </div>

    <!-- Assist Flag Badge (ABS / TC from ACC UDP) -->
    <div
      class="assist-badge micro-label"
      class:assist-brake-on={assistActive && channel === 'brake'}
      class:assist-tc-on={assistActive && channel === 'throttle'}
    >
      {assistLabel}: {assistActive ? 'ACTV' : 'STBY'}
    </div>
  </header>

  <!-- Main Meter + Readout Body -->
  <div class="meter-body">
    <!-- 20-Segment Vertical LED Bar (fills bottom to top) -->
    <div class="led-column" aria-label="{label} 20-segment vertical meter">
      {#each Array.from({ length: TOTAL_SEGMENTS }, (_, idx) => TOTAL_SEGMENTS - 1 - idx) as segIndex}
        <div
          class="led-segment"
          class:led-brake-on={channel === 'brake' && segIndex < activeCount}
          class:led-throttle-on={channel === 'throttle' && segIndex < activeCount}
        ></div>
      {/each}
    </div>

    <!-- Tabular Numeric Readouts -->
    <div class="readout-column">
      <div class="primary-readout">
        <span class="micro-label">DISPLACEMENT</span>
        <div
          class="display-value"
          style:color={channel === 'brake' ? 'var(--brand-brake)' : 'var(--brand-throttle)'}
        >
          {pctDisplay}<span class="pct-unit">%</span>
        </div>
      </div>

      <div class="derivative-box">
        <span class="micro-label">DERIVATIVE (d/dt)</span>
        <span class="sub-header derivative-val">
          {velPctPerSec >= 0 ? '+' : ''}{velPctPerSec}%/S
        </span>
      </div>

      <div class="derivative-box">
        <span class="micro-label">CHANNEL EARCON</span>
        <span class="sub-header">
          {channel === 'brake' ? 'LEFT // 300HZ' : 'RIGHT // 2.4KHZ'}
        </span>
      </div>
    </div>
  </div>

  <!-- Hardware Diagnostic & Alert Box (Design.md v2.0 §5.5) -->
  <footer
    class="diagnostic-box"
    class:diag-violation-brake={ (violationActive || assistActive) && channel === 'brake' }
    class:diag-violation-throttle={ (violationActive || assistActive) && channel === 'throttle' }
  >
    <div class="diag-top">
      <span class="micro-label">DIAGNOSTIC STATUS</span>
      <span class="micro-label">
        {violationActive || assistActive ? 'FAULT' : 'NOMINAL'}
      </span>
    </div>

    {#if channel === 'brake'}
      {#if violationActive}
        <div class="sub-header diag-alert-text">
          BRAKE DUMP DETECTED ({Math.round(violationVelocity)}%/S)
        </div>
      {:else if assistActive}
        <div class="sub-header diag-alert-text">
          ABS LOCK INTERVENTION ACTIVE
        </div>
      {:else}
        <div class="sub-header diag-nominal-text">
          SMOOTH PEDAL BLEED // PROGRESSIVE DECAY
        </div>
      {/if}
    {:else}
      {#if violationActive}
        <div class="sub-header diag-alert-text">
          THROTTLE SNAP DETECTED (+{Math.abs(Math.round(violationVelocity))}%/S)
        </div>
      {:else if assistActive}
        <div class="sub-header diag-alert-text">
          TC SLIP CUT INTERVENTION ACTIVE
        </div>
      {:else}
        <div class="sub-header diag-nominal-text">
          CONTROLLED ROLL-ON // TRACTION OPTIMAL
        </div>
      {/if}
    {/if}
  </footer>
</section>

<style>
  .trigger-panel {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--p-3);
    border-bottom: 1px solid var(--brand-border);
    background: var(--brand-elevated);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: var(--p-2);
  }

  .tag-box {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    padding: 3px 6px;
    color: var(--brand-black);
  }

  .tag-brake {
    background: var(--brand-brake);
  }

  .tag-throttle {
    background: var(--brand-throttle);
  }

  .assist-badge {
    padding: 4px 8px;
    border: 1px solid var(--brand-borderLight);
    background: var(--brand-black);
    color: var(--brand-muted);
  }

  .assist-brake-on {
    background: var(--brand-brake);
    border-color: var(--brand-brake);
    color: var(--brand-black);
  }

  .assist-tc-on {
    background: var(--brand-warning);
    border-color: var(--brand-warning);
    color: var(--brand-black);
  }

  .meter-body {
    display: grid;
    grid-template-columns: 44px 1fr;
    gap: var(--p-4);
    padding: var(--p-4);
    align-items: stretch;
  }

  /* 20 Stacked LED Segments: 4px height, 2px vertical gap (Design.md v2.0 §5.2) */
  .led-column {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 2px;
    background: var(--brand-black);
    padding: var(--p-1);
    border: 1px solid var(--brand-border);
  }

  .led-segment {
    width: 100%;
    height: 6px;
    min-height: 4px;
    background: #1A1A1F;
    transition: background-color 0.04s linear;
  }

  .led-brake-on {
    background: var(--brand-brake);
  }

  .led-throttle-on {
    background: var(--brand-throttle);
  }

  .readout-column {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: var(--p-3);
  }

  .primary-readout {
    display: flex;
    flex-direction: column;
    gap: var(--p-1);
  }

  .pct-unit {
    font-size: 20px;
    font-weight: 700;
    margin-left: 2px;
    color: var(--brand-muted);
  }

  .derivative-box {
    background: var(--brand-black);
    border: 1px solid var(--brand-border);
    padding: var(--p-2);
    display: flex;
    flex-direction: column;
    gap: var(--p-1);
  }

  .derivative-val {
    color: var(--brand-accent);
    font-weight: 700;
  }

  /* Hardware Diagnostic & Alert Box (Design.md v2.0 §5.5) */
  .diagnostic-box {
    margin: 0 var(--p-3) var(--p-3) var(--p-3);
    padding: var(--p-3);
    border: 1px solid var(--brand-border);
    background: var(--brand-black);
    display: flex;
    flex-direction: column;
    gap: var(--p-1);
  }

  .diag-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .diag-nominal-text {
    color: var(--brand-muted);
  }

  .diag-violation-brake {
    border: 1px solid var(--brand-brake);
    background: var(--brand-brakeBg);
  }

  .diag-violation-throttle {
    border: 1px solid var(--brand-warning);
    background: var(--brand-throttleBg);
  }

  .diag-alert-text {
    color: var(--brand-accent);
    font-weight: 700;
  }
</style>
