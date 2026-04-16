<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen, emit } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { allThemeClasses } from './themes.js';
  import { resolveOverlayCSS, styleHasFrost } from './overlayStyles.js';

  let audioLevel = $state(0);
  let elapsed = $state(0);
  let mode = $state('minimal'); // 'minimal' or 'full'
  let overlayStyle = $state('default');
  let overlayOpacity = $state(0.85);
  let currentTheme = $state('white');
  let noTransparency = $state(false);

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

  /** Build an inline style string from the current overlay style + theme. */
  let overlayInlineStyle = $derived(() => {
    const css = resolveOverlayCSS(overlayStyle, currentTheme, overlayOpacity);
    let s = `background: ${css.background}; border: ${css.border}; box-shadow: ${css.boxShadow};`;
    return s;
  });

  /** Whether the current style needs the frost noise texture. */
  let showFrost = $derived(() => styleHasFrost(overlayStyle));

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
          if (event.payload?.theme) {
            currentTheme = event.payload.theme;
            applyTheme(event.payload.theme);
          }
          if (event.payload?.mode) mode = event.payload.mode;
          if (event.payload?.overlayStyle) overlayStyle = event.payload.overlayStyle;
          if (typeof event.payload?.overlayOpacity === 'number') overlayOpacity = event.payload.overlayOpacity;
          if (typeof event.payload?.noTransparency === 'boolean') {
            noTransparency = event.payload.noTransparency;
            document.documentElement.style.setProperty(
              '--overlay-radius',
              event.payload.noTransparency ? '0px' : '40px'
            );
            if (event.payload.noTransparency) {
              // On systems without a compositor the window is opaque, so
              // match the html/body background to the overlay to avoid a
              // visible "frame" of the default window colour.
              const css = resolveOverlayCSS(overlayStyle, currentTheme, overlayOpacity);
              document.documentElement.style.setProperty('background', css.background, 'important');
              document.body.style.setProperty('background', css.background, 'important');
            } else {
              document.documentElement.style.setProperty('background', 'transparent', 'important');
              document.body.style.setProperty('background', 'transparent', 'important');
            }
          }
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

<!-- Hidden SVG noise filter used by frost styles -->
<svg class="frost-svg" aria-hidden="true">
  <filter id="frost-noise">
    <feTurbulence type="fractalNoise" baseFrequency="0.75" numOctaves="4" stitchTiles="stitch" result="noise"/>
    <feColorMatrix type="saturate" values="0" in="noise" result="mono"/>
    <feBlend in="SourceGraphic" in2="mono" mode="overlay"/>
  </filter>
</svg>

{#if mode === 'minimal'}
  <div class="overlay minimal" class:neon-glow={overlayStyle === 'neon_glow'} class:no-transparency={noTransparency} style={overlayInlineStyle()} data-tauri-drag-region>
    {#if showFrost() && !noTransparency}<div class="frost-texture" data-tauri-drag-region></div>{/if}
    <div class="bars" data-tauri-drag-region>
      {#each bars() as height}
        <div class="bar" style="height: {height * 100}%"></div>
      {/each}
    </div>
  </div>
{:else}
  <div class="overlay full" class:neon-glow={overlayStyle === 'neon_glow'} class:no-transparency={noTransparency} style={overlayInlineStyle()} data-tauri-drag-region>
    {#if showFrost() && !noTransparency}<div class="frost-texture" data-tauri-drag-region></div>{/if}
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
  /* Hidden SVG for the frost noise filter definition */
  .frost-svg {
    position: absolute;
    width: 0;
    height: 0;
    pointer-events: none;
  }

  .overlay {
    display: flex;
    align-items: center;
    border-radius: 40px;
    user-select: none;
    cursor: grab;
    overflow: hidden;
    position: relative;
    /* Fallback values — overridden by inline style from overlayStyles.js */
    background: var(--bg-secondary, #eef2f7);
    border: none;
    transition: box-shadow 0.3s ease, background 0.3s ease;
  }

  /* Frost noise grain overlay — sits behind content, on top of background */
  .frost-texture {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    filter: url(#frost-noise);
    opacity: 0.12;
    pointer-events: none;
    z-index: 0;
  }

  /* Ensure all content sits above the frost texture */
  .overlay > :not(.frost-texture) {
    position: relative;
    z-index: 1;
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

  /* Neon glow breathing animation */
  .overlay.neon-glow {
    animation: neon-breathe 1.4s ease-in-out infinite;
  }
  @keyframes neon-breathe {
    0%, 100% {
      box-shadow: 0 0 14px rgba(37,99,235,0.2), 0 0 30px rgba(37,99,235,0.08), inset 0 0 10px rgba(37,99,235,0.05);
    }
    50% {
      box-shadow: 0 0 22px rgba(37,99,235,0.35), 0 0 44px rgba(37,99,235,0.14), inset 0 0 16px rgba(37,99,235,0.09);
    }
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
    background: var(--accent, #2563eb);
    box-shadow: 0 0 5px var(--accent-glow, rgba(37, 99, 235, 0.3));
    animation: dot-blink 1.4s ease-in-out infinite;
  }

  @keyframes dot-blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .label {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 1px;
    color: var(--accent, #2563eb);
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
    background: var(--accent, #2563eb);
    color: var(--on-accent, #fff);
    cursor: pointer;
    box-shadow: 0 1px 4px var(--accent-glow, rgba(37, 99, 235, 0.3));
  }

  .stop-btn:hover {
    background: var(--accent-hover, #1d4ed8);
    box-shadow: 0 2px 8px var(--accent-glow, rgba(37, 99, 235, 0.4));
  }

  .stop-btn:active {
    opacity: 0.85;
    transform: scale(0.92);
  }

  /* Rectangle fallback for systems without a compositor (e.g. Linux X11) */
  .overlay.no-transparency {
    border-radius: 0;
    box-shadow: none !important;
  }
</style>
