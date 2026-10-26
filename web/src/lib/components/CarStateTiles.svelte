<script lang="ts">
  interface Props {
    speedKmh: number;
    gear: number;
    modulationScore: number;
    lapIndex: number;
    lapTimeMs: number;
  }

  let { speedKmh, gear, modulationScore, lapIndex, lapTimeMs }: Props = $props();

  let gearGlyph = $derived.by(() => {
    if (gear < 0) return 'R';
    if (gear === 0) return 'N';
    return String(gear);
  });

  let formattedLap = $derived(`#${String(Math.max(1, lapIndex)).padStart(2, '0')}`);

  let formattedLapTime = $derived.by(() => {
    const totalSec = Math.floor(lapTimeMs / 1000);
    const mins = Math.floor(totalSec / 60);
    const secs = totalSec % 60;
    const tenths = Math.floor((lapTimeMs % 1000) / 100);
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${tenths}`;
  });
</script>

<!-- Four equal-width square-edged tiles beneath the oscilloscope (Design.md v2.0 §5.4) -->
<section class="tiles-grid" aria-label="High-Density Car State Tiles">
  <!-- 1. Speed Tile -->
  <div class="state-tile">
    <span class="micro-label">VEHICLE SPEED</span>
    <div class="tile-main">
      <span class="metric-medium">{Math.round(speedKmh)}</span>
      <span class="micro-label">KM/H</span>
    </div>
  </div>

  <!-- 2. Gear Tile -->
  <div class="state-tile">
    <span class="micro-label">GEAR SELECTION</span>
    <div class="tile-main">
      <span class="metric-medium gear-val">{gearGlyph}</span>
      <span class="micro-label">SEQ</span>
    </div>
  </div>

  <!-- 3. Modulation Score Tile -->
  <div class="state-tile">
    <span class="micro-label">MODULATION SCORE</span>
    <div class="tile-main">
      <span
        class="metric-medium"
        style:color={modulationScore >= 80
          ? 'var(--brand-throttle)'
          : modulationScore >= 60
            ? 'var(--brand-warning)'
            : 'var(--brand-brake)'}
      >
        {modulationScore.toFixed(1)}%
      </span>
      <span class="micro-label">IDX</span>
    </div>
  </div>

  <!-- 4. Lap Index Tile -->
  <div class="state-tile">
    <span class="micro-label">LAP INDEX</span>
    <div class="tile-main">
      <span class="metric-medium">{formattedLap}</span>
      <span class="micro-label">{formattedLapTime}</span>
    </div>
  </div>
</section>

<style>
  .tiles-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--p-3);
  }

  .state-tile {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    padding: var(--p-3);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: var(--p-2);
    min-height: 82px;
  }

  .tile-main {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .gear-val {
    color: var(--brand-accent);
  }

  @media (max-width: 640px) {
    .tiles-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
