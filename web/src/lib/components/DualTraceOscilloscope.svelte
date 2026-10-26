<script lang="ts">
  import { onMount } from 'svelte';

  export interface DualSamplePoint {
    t: number;
    brk: number;
    thr: number;
  }

  interface Props {
    history: DualSamplePoint[];
    liveBrake: number;
    liveThrottle: number;
  }

  let { history, liveBrake, liveThrottle }: Props = $props();

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
        const padLeft = 36;
        const padRight = 10;
        const padTop = 14;
        const padBottom = 22;
        const plotW = Math.max(20, width - padLeft - padRight);
        const plotH = Math.max(20, height - padTop - padBottom);

        // 1. Base Canvas Fill (#080808 brand-black)
        ctx.fillStyle = '#080808';
        ctx.fillRect(0, 0, width, height);

        // 2. 4 Horizontal Equidistant Grid Divisions: 0%, 25%, 50%, 75%, 100% (Design.md v2.0 §5.3)
        ctx.strokeStyle = '#242429';
        ctx.lineWidth = 1;
        ctx.fillStyle = '#70707D';
        ctx.font = '700 10px "JetBrains Mono", monospace';

        const yLevels = [0.0, 0.25, 0.5, 0.75, 1.0];
        ctx.beginPath();
        for (const lvl of yLevels) {
          const y = Math.floor(padTop + (1.0 - lvl) * plotH) + 0.5;
          ctx.moveTo(padLeft, y);
          ctx.lineTo(padLeft + plotW, y);
          ctx.fillText(`${Math.round(lvl * 100)}%`, 4, y + 3);
        }

        // 3. 6 Vertical Time Divisions (0.5s intervals across 3.0s window)
        const timeDivs = 6;
        for (let i = 0; i <= timeDivs; i++) {
          const u = i / timeDivs;
          const x = Math.floor(padLeft + u * plotW) + 0.5;
          if (i < timeDivs) {
            ctx.moveTo(x, padTop);
            ctx.lineTo(x, padTop + plotH);
          }
          const secLabel = -3.0 + i * 0.5;
          const labelStr = i === timeDivs ? '0.0S' : `${secLabel.toFixed(1)}S`;
          const textX = i === timeDivs ? x - 26 : Math.max(padLeft, x - 14);
          ctx.fillText(labelStr, textX, height - 6);
        }
        ctx.stroke();

        // 4. Live Hit-Line at right edge: 1px vertical dashed cursor [2, 2] (Design.md v2.0 §5.3)
        const hitX = Math.floor(padLeft + plotW) - 0.5;
        ctx.save();
        ctx.setLineDash([2, 2]);
        ctx.strokeStyle = '#EDEDED';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(hitX, padTop);
        ctx.lineTo(hitX, padTop + plotH);
        ctx.stroke();
        ctx.restore();

        const now = performance.now();
        const windowMs = 3000;

        if (history.length > 1) {
          // 5. Brake Trace: #FF3333 solid 2px line in single contiguous stroke (Design.md v2.0 §5.3 & §6.3)
          ctx.beginPath();
          ctx.strokeStyle = '#FF3333';
          ctx.lineWidth = 2;
          let startedBrk = false;
          for (let i = 0; i < history.length; i++) {
            const pt = history[i];
            const age = now - pt.t;
            if (age > windowMs) continue;
            const x = padLeft + plotW * (1.0 - age / windowMs);
            const y = padTop + (1.0 - pt.brk) * plotH;
            if (!startedBrk) {
              ctx.moveTo(x, y);
              startedBrk = true;
            } else {
              ctx.lineTo(x, y);
            }
          }
          ctx.stroke();

          // 6. Throttle Trace: #00E676 solid 2px line in single contiguous stroke
          ctx.beginPath();
          ctx.strokeStyle = '#00E676';
          ctx.lineWidth = 2;
          let startedThr = false;
          for (let i = 0; i < history.length; i++) {
            const pt = history[i];
            const age = now - pt.t;
            if (age > windowMs) continue;
            const x = padLeft + plotW * (1.0 - age / windowMs);
            const y = padTop + (1.0 - pt.thr) * plotH;
            if (!startedThr) {
              ctx.moveTo(x, y);
              startedThr = true;
            } else {
              ctx.lineTo(x, y);
            }
          }
          ctx.stroke();
        }

        // 7. Square Hit-Line Indicators for Live Brake & Throttle
        const brkY = padTop + (1.0 - liveBrake) * plotH;
        const thrY = padTop + (1.0 - liveThrottle) * plotH;
        ctx.fillStyle = '#FF3333';
        ctx.fillRect(hitX - 3, brkY - 3, 6, 6);
        ctx.fillStyle = '#00E676';
        ctx.fillRect(hitX - 3, thrY - 3, 6, 6);

        ctx.restore();
      }
      animId = requestAnimationFrame(render);
    };

    animId = requestAnimationFrame(render);
    return () => cancelAnimationFrame(animId);
  });
</script>

<section class="scope-panel">
  <header class="scope-header">
    <div class="scope-title-group">
      <span class="header-section">LIVE DUAL-TRACE OSCILLOSCOPE</span>
      <span class="micro-label">WINDOW: 3.0S // 0.5S DIV // 120 FPS</span>
    </div>

    <div class="scope-legend">
      <span class="legend-item micro-label">
        <span class="swatch swatch-brake"></span> BRAKE (LT)
      </span>
      <span class="legend-item micro-label">
        <span class="swatch swatch-throttle"></span> THROTTLE (RT)
      </span>
    </div>
  </header>

  <div class="scope-canvas-box">
    <canvas bind:this={canvasEl}></canvas>
  </div>
</section>

<style>
  .scope-panel {
    background: var(--brand-surface);
    border: 1px solid var(--brand-border);
    display: flex;
    flex-direction: column;
  }

  .scope-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--p-3);
    background: var(--brand-elevated);
    border-bottom: 1px solid var(--brand-border);
    flex-wrap: wrap;
    gap: var(--p-2);
  }

  .scope-title-group {
    display: flex;
    align-items: baseline;
    gap: var(--p-3);
  }

  .scope-legend {
    display: flex;
    align-items: center;
    gap: var(--p-3);
  }

  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: var(--p-1);
    color: var(--brand-accent);
  }

  .swatch {
    width: 8px;
    height: 8px;
    display: inline-block;
  }

  .swatch-brake {
    background: var(--brand-brake);
  }

  .swatch-throttle {
    background: var(--brand-throttle);
  }

  .scope-canvas-box {
    width: 100%;
    height: 230px;
    background: var(--brand-black);
  }

  canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
