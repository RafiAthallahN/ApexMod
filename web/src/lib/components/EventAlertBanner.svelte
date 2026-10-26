<script lang="ts">
  import type { ModulationEventType } from '../types';

  interface Props {
    activeEvent: ModulationEventType;
    velocityPctPerSec: number;
  }

  let { activeEvent, velocityPctPerSec }: Props = $props();
</script>

<div class="alert-slot">
  {#if activeEvent === 'BRAKE_DUMP'}
    <div class="alert-capsule alert-dump" role="status">
      <span class="dot"></span>
      <span class="label">
        BRAKE DUMP DETECTED ({velocityPctPerSec >= 0 ? '-' : ''}{Math.abs(Math.round(velocityPctPerSec))}%/s)
      </span>
    </div>
  {:else if activeEvent === 'THROTTLE_SNAP'}
    <div class="alert-capsule alert-snap" role="status">
      <span class="dot"></span>
      <span class="label">
        THROTTLE SNAP DETECTED (+{Math.abs(Math.round(velocityPctPerSec))}%/s)
      </span>
    </div>
  {:else}
    <div class="alert-capsule alert-idle">
      <span class="dot-idle"></span>
      <span class="caption-unit">EARCON ENGINE READY · STEREO L/R</span>
    </div>
  {/if}
</div>

<style>
  .alert-slot {
    display: flex;
    justify-content: center;
    align-items: center;
    min-height: 40px;
  }

  .alert-capsule {
    display: inline-flex;
    align-items: center;
    gap: var(--space-xs);
    padding: 6px var(--space-sm);
    border-radius: 999px;
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    transition: all 150ms ease-out;
  }

  .alert-idle {
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
  }

  .dot-idle {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-throttle);
    opacity: 0.7;
  }

  .alert-dump {
    background: rgba(255, 59, 48, 0.18);
    border: 1px solid var(--accent-brake);
    color: var(--text-primary);
    box-shadow: 0 0 24px rgba(255, 59, 48, 0.55);
    animation: pulse-alert 400ms ease-out;
  }

  .alert-dump .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-brake);
  }

  .alert-snap {
    background: rgba(255, 149, 0, 0.18);
    border: 1px solid var(--status-warning);
    color: var(--text-primary);
    box-shadow: 0 0 24px rgba(255, 149, 0, 0.55);
    animation: pulse-alert 400ms ease-out;
  }

  .alert-snap .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--status-warning);
  }

  @keyframes pulse-alert {
    0% {
      transform: scale(0.96);
      opacity: 0.7;
    }
    40% {
      transform: scale(1.03);
      opacity: 1;
    }
    100% {
      transform: scale(1);
      opacity: 1;
    }
  }
</style>
