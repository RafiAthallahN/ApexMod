<script lang="ts">
  import { onMount } from 'svelte';

  interface SamplePoint {
    t: number;
    val: number;
  }

  interface Props {
    channel: 'brake' | 'throttle';
    label: string;
    liveValue: number;
    liveVelocity: number;
    history: SamplePoint[];
    violationActive: boolean;
  }

  let { channel, label, liveValue, liveVelocity, history, violationActive }: Props = $props();

  let canvasEl: HTMLCanvasElement;

  onMount(() => {
    let animId = 0;
    const ctx = canvasEl.getContext('2d');

    const render = () => {
      if (ctx && canvasEl) {
        const dpr = window.devicePixelRatio || 1;
        const rect = canvasEl.getBoundingClientRect();
        const w = Math.floor(rect.width * dpr);
        const h = Math.floor(rect.height * dpr);
        if (canvasEl.width !== w || canvasEl.height !== h) {
          canvasEl.width = w;
          canvasEl.height = h;
        }

        ctx.save();
        ctx.scale(dpr, dpr);
        const width = rect.width;
        const height = rect.height;
        const padTop = 14;
        const padBottom = 22;
        const plotH = Math.max(10, height - padTop - padBottom);

        // Clear background
        ctx.fillStyle = '#16181D';
        ctx.fillRect(0, 0, width, height);

        // Horizontal grid lines (0%, 50%, 70%/20% threshold, 100%)
        ctx.strokeStyle = '#2D313B';
        ctx.lineWidth = 1;
        for (const pct of [0.0, 0.5, 1.0]) {
          const y = padTop + (1.0 - pct) * plotH;
          ctx.beginPath();
          ctx.moveTo(0, y);
          ctx.lineTo(width, y);
          ctx.stroke();
        }

        // Target Envelope (Design.md §5 Component A)
        // Ideal progressive trail-braking decay or progressive throttle ramp reference band
        ctx.beginPath();
        const envColor = channel === 'brake' ? 'rgba(74, 21, 19, 0.45)' : 'rgba(17, 56, 27, 0.45)';
        ctx.fillStyle = envColor;
        const steps = 40;
        for (let i = 0; i <= steps; i++) {
          const u = i / steps;
          const x = u * width;
          const upper =
            channel === 'brake'
              ? 0.95 * Math.pow(1.0 - u * 0.85, 1.35)
              : Math.pow(u, 1.25) * 0.95;
          const y = padTop + (1.0 - upper) * plotH;
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        for (let i = steps; i >= 0; i--) {
          const u = i / steps;
          const x = u * width;
          const lower =
            channel === 'brake'
              ? Math.max(0, 0.72 * Math.pow(1.0 - u * 0.95, 1.85))
              : Math.max(0, Math.pow(u, 1.65) * 0.72);
          const y = padTop + (1.0 - lower) * plotH;
          ctx.lineTo(x, y);
        }
        ctx.closePath();
        ctx.fill();

        // Draw 3.0s scrolling trace
        const now = performance.now();
        const windowMs = 3000;
        if (history.length > 1) {
          ctx.beginPath();
          ctx.strokeStyle = channel === 'brake' ? '#FF3B30' : '#34C759';
          ctx.lineWidth = 2.5;

          // Accessibility (Design.md §7): micro-dashed for Brake, solid for Throttle
          if (channel === 'brake') {
            ctx.setLineDash([7, 3]);
          } else {
            ctx.setLineDash([]);
          }

          let started = false;
          for (let i = 0; i < history.length; i++) {
            const pt = history[i];
            const age = now - pt.t;
            if (age > windowMs) continue;
            const x = width * (1.0 - age / windowMs);
            const y = padTop + (1.0 - pt.val) * plotH;
            if (!started) {
              ctx.moveTo(x, y);
              started = true;
            } else {
              ctx.lineTo(x, y);
            }
          }
          ctx.stroke();
          ctx.setLineDash([]);
        }

        // Right-edge live hit-line marker
        const liveY = padTop + (1.0 - liveValue) * plotH;
        ctx.fillStyle = channel === 'brake' ? '#FF3B30' : '#34C759';
        ctx.beginPath();
        ctx.arc(width - 4, liveY, 4, 0, Math.PI * 2);
        ctx.fill();

        // Axis markers (Caption / Unit 11px Mono)
        ctx.fillStyle = '#8C93A0';
        ctx.font = '500 11px "JetBrains Mono", monospace';
        ctx.fillText('<- 3.0s', 8, height - 6);
        ctx.fillText('Live (0s)', width - 68, height - 6);

        ctx.restore();
      }
      animId = requestAnimationFrame(render);
    };

    animId = requestAnimationFrame(render);
    return () => cancelAnimationFrame(animId);
  });
</script>

<section
  class="ribbon-card"
  class:violation-brake={violationActive && channel === 'brake'}
  class:violation-throttle={violationActive && channel === 'throttle'}
>
  <header class="ribbon-header">
    <div class="channel-title">
      <span
        class="channel-badge"
        class:badge-brake={channel === 'brake'}
        class:badge-throttle={channel === 'throttle'}
      >
        {channel === 'brake' ? 'L-CH' : 'R-CH'}
      </span>
      <span class="body-regular title-text">{label}</span>
    </div>

    <div class="channel-metrics">
      <span class="caption-unit">
        {liveVelocity >= 0 ? '+' : ''}{Math.round(liveVelocity * 100)}%/s
      </span>
      <span
        class="metric-primary"
        style:color={channel === 'brake' ? 'var(--accent-brake)' : 'var(--accent-throttle)'}
      >
        {Math.round(liveValue * 100)}%
      </span>
    </div>
  </header>

  <div class="canvas-wrap">
    <canvas bind:this={canvasEl}></canvas>
  </div>
</section>

<style>
  .ribbon-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: var(--space-sm);
    display: flex;
    flex-direction: column;
    gap: var(--space-xs);
    transition: border-color 200ms ease-out, box-shadow 200ms ease-out;
  }

  /* Violation State (Design.md §6): Border glow with 200ms fade-out */
  .violation-brake {
    border-color: var(--accent-brake);
    box-shadow: 0 0 20px rgba(255, 59, 48, 0.45);
  }

  .violation-throttle {
    border-color: var(--status-critical);
    box-shadow: 0 0 20px rgba(255, 45, 85, 0.45);
  }

  .ribbon-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .channel-title {
    display: flex;
    align-items: center;
    gap: var(--space-xs);
  }

  .title-text {
    font-weight: 600;
    letter-spacing: 0.03em;
  }

  .channel-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .badge-brake {
    background: var(--accent-brake-dim);
    color: var(--accent-brake);
  }

  .badge-throttle {
    background: var(--accent-throttle-dim);
    color: var(--accent-throttle);
  }

  .channel-metrics {
    display: flex;
    align-items: baseline;
    gap: var(--space-sm);
  }

  .canvas-wrap {
    width: 100%;
    height: 180px;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
  }

  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
