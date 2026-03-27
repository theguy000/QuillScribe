<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen, emit } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { allThemeClasses } from './themes.js';

  let audioLevel = $state(0);
  let elapsed = $state(0);
  let mode = $state('minimal'); // 'minimal' or 'full'

  let audioInterval = null;
  let timerInterval = null;

  const BAR_COUNT_MINIMAL = 16;
  const BAR_COUNT_FULL = 20;

  function generateBars(raw, count) {
    const boosted = Math.min(1, raw * 6);
    const base = boosted > 0 ? Math.pow(boosted, 0.45) : 0;
    const result = [];
    for (let i = 0; i < count; i++) {
      const center = (count - 1) / 2;
      const dist = Math.abs(i - center) / center;
      const wave = 1 - dist * 0.5;
      const seed = Math.sin(i * 2.39996) * 0.5 + 0.5;
      const offset = 0.6 + seed * 0.8;
      const h = base * wave * offset;
      result.push(Math.min(1, Math.max(0.05, h)));
    }
    return result;
  }

  let bars = $derived(() => {
    const count = mode === 'minimal' ? BAR_COUNT_MINIMAL : BAR_COUNT_FULL;
    return generateBars(audioLevel, count);
  });

  function startPolling() {
    if (audioInterval) return;
    audioInterval = setInterval(async () => {
      try {
        audioLevel = await invoke('get_audio_level');
      } catch {
        audioLevel = 0;
      }
    }, 35);
  }

  function stopPolling() {
    if (audioInterval) {
      clearInterval(audioInterval);
      audioInterval = null;
    }
    audioLevel = 0;
  }

  function startTimer(fromSecs = 0) {
    elapsed = fromSecs;
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = setInterval(() => { elapsed += 1; }, 1000);
  }

  function stopTimer() {
    if (timerInterval) {
      clearInterval(timerInterval);
      timerInterval = null;
    }
  }

  function formatTime(secs) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  async function handleStop() {
    await emit('overlay-stop-recording');
  }

  function applyTheme(theme) {
    if (!theme) return;
    const cl = document.documentElement.classList;
    cl.remove('dark', ...allThemeClasses);
    const isDark = theme.startsWith('dark_') || theme === 'obsidian';
    if (isDark) cl.add('dark');
    cl.add(theme);
  }

  onMount(() => {
    /** @type {Array<() => void>} */
    let unlisteners = [];

    (async () => {
      unlisteners.push(
        await listen('overlay-show', (event) => {
          if (event.payload?.theme) applyTheme(event.payload.theme);
          if (event.payload?.mode) mode = event.payload.mode;
          const startFrom = typeof event.payload?.elapsed === 'number'
            ? event.payload.elapsed
            : 0;
          startPolling();
          if (mode === 'full') startTimer(startFrom);
        }),

        await listen('overlay-hide', () => {
          stopPolling();
          stopTimer();
        }),
      );
    })();

    return () => {
      stopPolling();
      stopTimer();
      for (const unlisten of unlisteners) {
        unlisten?.();
      }
    };
  });
</script>

{#if mode === 'minimal'}
  <div class="overlay minimal" data-tauri-drag-region>
    <div class="bars" data-tauri-drag-region>
      {#each bars() as height}
        <div class="bar" style="height: {height * 100}%"></div>
      {/each}
    </div>
  </div>
{:else}
  <div class="overlay full" data-tauri-drag-region>
    <div class="indicator" data-tauri-drag-region>
      <span class="red-dot"></span>
      <span class="label" data-tauri-drag-region>REC</span>
      <span class="time" data-tauri-drag-region>{formatTime(elapsed)}</span>
    </div>
    <div class="bars full-bars" data-tauri-drag-region>
      {#each bars() as height}
        <div class="bar" style="height: {height * 100}%"></div>
      {/each}
    </div>
    <button class="stop-btn" onclick={handleStop} title="Stop recording">
      <svg width="10" height="10" viewBox="0 0 12 12">
        <rect x="1" y="1" width="10" height="10" rx="1.5" fill="currentColor"/>
      </svg>
    </button>
  </div>
{/if}

<style>
  .overlay {
    display: flex;
    align-items: center;
    background: var(--bg-secondary, #eef2f7);
    border: none;
    border-radius: 40px;
    user-select: none;
    cursor: grab;
    overflow: hidden;
  }

  .overlay.minimal {
    justify-content: center;
    width: 100vw;
    height: 100vh;
    padding: 0 6px;
  }

  .overlay.full {
    gap: 10px;
    width: 100vw;
    height: 100vh;
    padding: 0 12px;
  }

  .bars {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 20px;
    flex: 1;
    justify-content: center;
  }

  .full-bars {
    height: 22px;
    align-items: flex-end;
    gap: 1.5px;
  }

  .bar {
    width: 2px;
    min-height: 2px;
    border-radius: 1px;
    background: var(--accent, #2563eb);
    transition: height 0.04s linear;
  }

  .full-bars .bar {
    width: 2px;
  }

  .indicator {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
  }

  .red-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--danger, #e81123);
    box-shadow: 0 0 5px var(--recording-glow, rgba(231, 76, 60, 0.5));
  }

  .label {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 1px;
    color: var(--danger, #e81123);
    text-transform: uppercase;
  }

  .time {
    font-size: 10px;
    font-weight: 600;
    color: var(--text-primary, #162033);
    font-variant-numeric: tabular-nums;
    min-width: 28px;
  }

  .stop-btn {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 6px;
    background: var(--danger, #e81123);
    color: var(--on-danger, #fff);
    cursor: pointer;
  }

  .stop-btn:hover {
    background: var(--danger-hover, #c82333);
  }

  .stop-btn:active {
    opacity: 0.85;
  }
</style>
