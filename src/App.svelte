<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import TitleBar from './lib/TitleBar.svelte';

  // App state
  let isRecording = $state(false);
  let transcriptionText = $state('');
  let statusMessage = $state('Ready');
  let showSettings = $state(false);
  let currentTheme = $state('light');
  let compactMode = $state(false);
  let audioLevel = $state(0);

  let audioLevelInterval = null;

  let isDark = $derived(currentTheme === 'dark');

  // Apply theme to document
  $effect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  });

  // Poll audio level while recording
  $effect(() => {
    if (isRecording) {
      audioLevelInterval = setInterval(async () => {
        try {
          audioLevel = await invoke('get_audio_level');
        } catch {
          audioLevel = 0;
        }
      }, 50);
    } else {
      if (audioLevelInterval) {
        clearInterval(audioLevelInterval);
        audioLevelInterval = null;
      }
      audioLevel = 0;
    }

    return () => {
      if (audioLevelInterval) {
        clearInterval(audioLevelInterval);
        audioLevelInterval = null;
      }
    };
  });

  onMount(async () => {
    try {
      const settings = await invoke('get_settings');
      if (settings) {
        currentTheme = settings.theme || 'light';
        compactMode = settings.compact_mode || false;
      }
    } catch (err) {
      console.warn('Failed to load settings:', err);
    }
  });

  async function toggleRecording() {
    if (!isRecording) {
      try {
        statusMessage = 'Starting recording...';
        await invoke('start_recording');
        isRecording = true;
        statusMessage = 'Recording... Click to stop';
        transcriptionText = '';
      } catch (err) {
        statusMessage = `Error: ${err}`;
        isRecording = false;
      }
    } else {
      try {
        statusMessage = 'Transcribing...';
        isRecording = false;
        const text = await invoke('stop_recording');
        transcriptionText = text || '';
        statusMessage = text ? 'Transcription complete' : 'No speech detected';
      } catch (err) {
        statusMessage = `Error: ${err}`;
      }
    }
  }

  function openSettings() {
    showSettings = true;
  }

  function closeSettings() {
    showSettings = false;
  }
</script>

<TitleBar {isDark} />

<main class="app-container" class:compact={compactMode}>
  <div class="content">
    <!-- Microphone area -->
    <div class="mic-section">
      <button
        class="mic-button"
        class:recording={isRecording}
        onclick={toggleRecording}
        aria-label={isRecording ? 'Stop recording' : 'Start recording'}
      >
        <div
          class="mic-glow"
          style:opacity={isRecording ? 0.6 + audioLevel * 0.4 : 0}
          style:transform="scale({isRecording ? 1 + audioLevel * 0.5 : 0.8})"
        ></div>
        <div
          class="mic-ring"
          class:recording={isRecording}
          style:transform="scale({isRecording ? 1 + audioLevel * 0.15 : 1})"
        >
          <svg
            width="32"
            height="32"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <rect x="9" y="1" width="6" height="14" rx="3" />
            <path d="M19 10v1a7 7 0 0 1-14 0v-1" />
            <line x1="12" y1="19" x2="12" y2="23" />
            <line x1="8" y1="23" x2="16" y2="23" />
          </svg>
        </div>
      </button>
    </div>

    <!-- Status -->
    <div class="status-section">
      <p class="status-text" class:recording={isRecording}>{statusMessage}</p>
    </div>

    <!-- Transcription output -->
    {#if transcriptionText}
      <div class="transcription-section">
        <div class="transcription-box">
          <p class="transcription-label">Transcription</p>
          <p class="transcription-text">{transcriptionText}</p>
        </div>
      </div>
    {/if}
  </div>

  <!-- Bottom bar -->
  <div class="bottom-bar">
    <button class="settings-btn" onclick={openSettings}>
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="12" cy="12" r="3" />
        <path
          d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1
             -2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65
             0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0
             9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1
             -2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65
             0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0
             4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1
             2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65
             0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0
             1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1
             2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65
             0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0
             -1.51 1z"
        />
      </svg>
      Settings
    </button>
  </div>
</main>

<style>
  .app-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary);
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    padding: 24px;
  }

  .compact .content {
    gap: 16px;
    padding: 16px;
  }

  /* Microphone button */
  .mic-section {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .mic-button {
    position: relative;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 120px;
    height: 120px;
  }

  .mic-glow {
    position: absolute;
    width: 140px;
    height: 140px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--recording-glow) 0%, transparent 70%);
    transition: opacity 0.15s ease, transform 0.15s ease;
    pointer-events: none;
  }

  .mic-ring {
    width: 80px;
    height: 80px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-secondary);
    border: 2px solid var(--border);
    color: var(--text-secondary);
    transition: all 0.2s ease;
    position: relative;
    z-index: 1;
  }

  .mic-ring.recording {
    background: var(--accent);
    border-color: var(--accent);
    color: #ffffff;
    box-shadow: 0 0 20px var(--accent-glow);
  }

  .mic-button:hover .mic-ring:not(.recording) {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--bg-tertiary);
  }

  /* Status */
  .status-section {
    text-align: center;
  }

  .status-text {
    font-size: 13px;
    color: var(--text-secondary);
    transition: color 0.2s ease;
  }

  .status-text.recording {
    color: var(--accent);
    font-weight: 500;
  }

  /* Transcription */
  .transcription-section {
    width: 100%;
    max-width: 480px;
  }

  .transcription-box {
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 10px;
    padding: 16px;
    max-height: 180px;
    overflow-y: auto;
  }

  .transcription-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .transcription-text {
    font-size: 14px;
    line-height: 1.6;
    color: var(--text-primary);
    user-select: text;
    word-wrap: break-word;
  }

  /* Bottom bar */
  .bottom-bar {
    display: flex;
    justify-content: center;
    padding: 12px 16px;
    border-top: 1px solid var(--border-light);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .settings-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--border);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .settings-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--accent);
  }
</style>
