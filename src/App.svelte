<script>
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';

  let isRecording = $state(false);
  let transcriptionText = $state('');
  let statusMessage = $state('Ready');
  let currentTheme = $state('white');
  let compactMode = $state(false);
  let audioLevel = $state(0);
  let settings = $state(null);
  let activePanel = $state('record');
  let historyEntries = $state([]);
  let historyLoading = $state(false);
  let historyError = $state('');

  let audioLevelInterval = null;

  const historyDays = 30;

  let isDark = $derived(
    currentTheme === 'dark' ||
    currentTheme.startsWith('dark_') ||
    currentTheme === 'obsidian'
  );

  let recentHistory = $derived([...historyEntries].reverse());

  $effect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  });

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
    await Promise.all([loadSettings(), loadHistory()]);
  });

  async function loadSettings() {
    try {
      const loadedSettings = await invoke('get_settings');
      if (loadedSettings) {
        settings = loadedSettings;
        currentTheme = loadedSettings.ui?.theme || 'white';
        compactMode = loadedSettings.ui?.compact_mode || false;
      }
    } catch (err) {
      console.warn('Failed to load settings:', err);
    }
  }

  async function loadHistory() {
    historyLoading = true;
    historyError = '';

    try {
      historyEntries = await invoke('get_recent_history', { days: historyDays });
    } catch (err) {
      historyEntries = [];
      historyError = `Failed to load history: ${err}`;
    } finally {
      historyLoading = false;
    }
  }

  async function toggleRecording() {
    if (!isRecording) {
      try {
        activePanel = 'record';
        statusMessage = 'Starting recording...';
        await invoke('start_recording');
        isRecording = true;
        statusMessage = 'Recording... Click to stop';
        transcriptionText = '';
      } catch (err) {
        statusMessage = `Error: ${err}`;
        isRecording = false;
      }
      return;
    }

    try {
      statusMessage = 'Transcribing...';
      isRecording = false;
      const text = await invoke('stop_recording');
      transcriptionText = text || '';
      statusMessage = text ? 'Transcription complete' : 'No speech detected';
      await loadHistory();
    } catch (err) {
      statusMessage = `Error: ${err}`;
    }
  }

  function openSettings() {
    activePanel = 'settings';
  }

  function closeSettings() {
    activePanel = 'record';
  }

  function showRecordPanel() {
    activePanel = 'record';
  }

  async function showHistoryPanel() {
    activePanel = 'history';
    await loadHistory();
  }

  async function handleSaveSettings(newSettings) {
    try {
      await invoke('save_settings', { settings: newSettings });
      settings = newSettings;
      currentTheme = newSettings.ui?.theme || 'white';
      compactMode = newSettings.ui?.compact_mode || false;
    } catch (err) {
      console.error('Failed to save settings:', err);
      statusMessage = `Failed to save settings: ${err}`;
    }
  }

  function formatTimestamp(timestamp) {
    const date = new Date(timestamp);

    if (Number.isNaN(date.getTime())) {
      return timestamp;
    }

    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      hour: 'numeric',
      minute: '2-digit',
    }).format(date);
  }

  function formatDuration(seconds) {
    if (seconds == null || Number.isNaN(seconds)) return '—';
    if (seconds < 60) {
      return `${seconds.toFixed(1)}s`;
    }

    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = Math.round(seconds % 60)
      .toString()
      .padStart(2, '0');

    return `${minutes}:${remainingSeconds}`;
  }

  function formatConfidence(confidence) {
    if (confidence == null) return '—';
    return `${Math.round(confidence * 100)}%`;
  }
</script>

<TitleBar {isDark} />

<main class="app-container" class:compact={compactMode}>
  <aside class="sidebar">
    <div class="sidebar-header">
      <h1>QuillScribe</h1>
      <p class="sidebar-tagline">Voice Workspace</p>
    </div>

    <nav class="sidebar-nav">
      <p class="sidebar-section-label">Navigation</p>
      <button
        class="nav-button"
        class:active={activePanel === 'record'}
        onclick={showRecordPanel}
      >
        <span class="nav-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="9" y="1" width="6" height="14" rx="3" />
            <path d="M19 10v1a7 7 0 0 1-14 0v-1" />
            <line x1="12" y1="19" x2="12" y2="23" />
            <line x1="8" y1="23" x2="16" y2="23" />
          </svg>
        </span>
        <span class="nav-label">Record</span>
      </button>

      <button class="nav-button" class:active={activePanel === 'settings'} onclick={openSettings}>
        <span class="nav-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
        </span>
        <span class="nav-label">Settings</span>
      </button>

      <button
        class="nav-button"
        class:active={activePanel === 'history'}
        onclick={showHistoryPanel}
      >
        <span class="nav-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 3v5h5" />
            <path d="M3.05 13A9 9 0 1 0 6 5.3L3 8" />
            <path d="M12 7v5l3 3" />
          </svg>
        </span>
        <span class="nav-label">History</span>
      </button>
    </nav>

    <div class="sidebar-status-card">
      <div class="sidebar-status-row">
        <span class="status-dot" class:recording={isRecording}></span>
        <p class="sidebar-status" class:recording={isRecording}>{statusMessage}</p>
      </div>
      <p class="sidebar-meta">{historyEntries.length} sessions &middot; {historyDays}d</p>
    </div>
  </aside>

  <section class="workspace">
    {#if activePanel === 'history'}
      <div class="panel history-panel">
        <div class="panel-header">
          <div>
            <p class="panel-kicker">Archive</p>
            <h2>Recent history</h2>
          </div>
          <button class="panel-action" onclick={loadHistory} disabled={historyLoading}>
            {historyLoading ? 'Refreshing...' : 'Refresh'}
          </button>
        </div>

        {#if historyError}
          <div class="empty-state">
            <p class="empty-title">Could not load history</p>
            <p class="empty-copy">{historyError}</p>
          </div>
        {:else if historyLoading && recentHistory.length === 0}
          <div class="empty-state">
            <p class="empty-title">Loading history</p>
            <p class="empty-copy">Pulling recent sessions from the local statistics store.</p>
          </div>
        {:else if recentHistory.length === 0}
          <div class="empty-state">
            <p class="empty-title">No sessions yet</p>
            <p class="empty-copy">Start a recording and completed sessions will appear here.</p>
          </div>
        {:else}
          <div class="history-list">
            {#each recentHistory as entry}
              <article class="history-item">
                <div class="history-item-top">
                  <div>
                    <p class="history-time">{formatTimestamp(entry.timestamp)}</p>
                    <p class="history-mode">{entry.mode} mode</p>
                  </div>
                  <span class="history-badge" class:success={entry.success} class:failed={!entry.success}>
                    {entry.success ? 'Success' : 'Failed'}
                  </span>
                </div>
                <div class="history-metrics">
                  <span>{formatDuration(entry.duration_secs)} audio</span>
                  <span>{formatDuration(entry.transcription_time_secs)} transcribed</span>
                  <span>{entry.text_length} chars</span>
                  <span>{formatConfidence(entry.confidence)} confidence</span>
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </div>
    {:else if activePanel === 'settings'}
      <SettingsDialog
        show={activePanel === 'settings'}
        embedded={true}
        onclose={closeSettings}
        settings={settings}
        onsave={handleSaveSettings}
      />
    {:else}
      <div class="panel record-panel">
        <div class="panel-header">
          <div>
            <p class="panel-kicker">Recorder</p>
            <h2>Capture voice input</h2>
          </div>
          <p class="panel-summary">The left rail keeps controls close at hand while this panel stays focused on active work.</p>
        </div>

        <div class="record-grid">
          <section class="hero-card">
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

            <div class="hero-copy">
              <p class="hero-title">{isRecording ? 'Recording in progress' : 'Ready to record'}</p>
              <p class="hero-text">
                {isRecording
                  ? 'Click the microphone again to stop and send the captured audio for transcription.'
                  : 'Tap the microphone to start a fresh recording. The transcription result will appear in the panel beside it.'}
              </p>
            </div>
          </section>

          <section class="transcription-section">
            <div class="transcription-box">
              <div class="transcription-header">
                <p class="transcription-label">Main window</p>
                {#if transcriptionText}
                  <button class="panel-action subtle" onclick={showHistoryPanel}>View history</button>
                {/if}
              </div>

              {#if transcriptionText}
                <p class="transcription-text">{transcriptionText}</p>
              {:else}
                <div class="empty-state empty-inline">
                  <p class="empty-title">No transcription yet</p>
                  <p class="empty-copy">Once you stop a recording, the converted text will appear here.</p>
                </div>
              {/if}
            </div>
          </section>
        </div>
      </div>
    {/if}
  </section>
</main>

<style>
  .app-container {
    flex: 1;
    display: flex;
    overflow: hidden;
    background:
      radial-gradient(circle at top left, color-mix(in srgb, var(--accent) 7%, transparent) 0%, transparent 28%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg-secondary) 70%, transparent), transparent 24%),
      var(--bg-primary);
  }

  .sidebar {
    width: 220px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 20px 14px 16px;
    border-right: 1px solid var(--border-light);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .compact .sidebar {
    width: 200px;
    padding: 16px 12px;
    gap: 18px;
  }

  .sidebar-header {
    padding: 0 4px 16px;
    border-bottom: 1px solid var(--border-light);
  }

  .sidebar-header h1 {
    font-size: 17px;
    line-height: 1.2;
    letter-spacing: -0.01em;
    font-weight: 700;
    margin-bottom: 2px;
  }

  .sidebar-tagline {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);
    letter-spacing: 0.02em;
  }

  .sidebar-section-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    padding: 0 8px;
    margin-bottom: 4px;
  }

  .panel-kicker,
  .transcription-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-muted);
  }

  .sidebar-tagline,
  .panel-summary,
  .hero-text,
  .empty-copy,
  .sidebar-meta,
  .history-mode,
  .history-metrics {
    color: var(--text-secondary);
  }

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-button,
  .panel-action {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease;
  }

  .nav-button {
    width: 100%;
    padding: 9px 10px;
    justify-content: flex-start;
  }

  .panel-action {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-primary) 92%, transparent);
    color: var(--text-primary);
  }

  .nav-button:hover {
    background: color-mix(in srgb, var(--bg-primary) 80%, var(--bg-secondary));
    color: var(--text-primary);
  }

  .panel-action:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    background: color-mix(in srgb, var(--bg-primary) 98%, var(--accent) 2%);
  }

  .nav-button.active {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-primary));
    color: var(--accent);
  }

  .nav-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    opacity: 0.7;
  }

  .nav-button.active .nav-icon {
    opacity: 1;
  }

  .nav-label {
    font-size: 13px;
    font-weight: 500;
  }

  .nav-button.active .nav-label {
    font-weight: 600;
  }

  .sidebar-status-card {
    margin-top: auto;
    padding: 12px;
    border-radius: 10px;
    background: var(--bg-primary);
    border: 1px solid var(--border-light);
  }

  .sidebar-status-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }

  .status-dot.recording {
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent);
  }

  .sidebar-status {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .sidebar-status.recording {
    color: var(--accent);
  }

  .sidebar-meta {
    margin-top: 6px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-muted);
    padding-left: 15px;
  }

  .workspace {
    flex: 1;
    min-width: 0;
    padding: 22px;
    overflow: hidden;
  }

  .compact .workspace {
    padding: 16px;
  }

  .panel {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 26px;
    border: 1px solid color-mix(in srgb, var(--border-light) 92%, transparent);
    border-radius: 22px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 98%, transparent), color-mix(in srgb, var(--bg-secondary) 60%, transparent));
    box-shadow: 0 24px 60px color-mix(in srgb, var(--shadow-lg) 62%, transparent);
    overflow: hidden;
  }

  .compact .panel {
    padding: 18px;
    gap: 16px;
  }

  .panel-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding-bottom: 18px;
    border-bottom: 1px solid color-mix(in srgb, var(--border-light) 76%, transparent);
  }

  .panel-header h2 {
    font-size: 30px;
    line-height: 1.1;
    letter-spacing: -0.03em;
    margin-top: 8px;
  }

  .panel-summary {
    max-width: 300px;
    font-size: 13px;
    text-align: right;
    line-height: 1.6;
  }

  .panel-action {
    padding: 10px 14px;
    font-size: 13px;
    font-weight: 600;
  }

  .panel-action.subtle {
    padding: 0;
    border: none;
    background: transparent;
    color: var(--accent);
    transform: none;
  }

  .panel-action.subtle:hover {
    background: transparent;
    color: var(--accent-hover);
  }

  .record-grid {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(280px, 360px) minmax(0, 1fr);
    gap: 18px;
    min-height: 0;
  }

  .hero-card,
  .transcription-box,
  .history-item,
  .empty-state {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
  }

  .hero-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 22px;
    padding: 30px;
    text-align: center;
    background:
      radial-gradient(circle at top, color-mix(in srgb, var(--accent) 18%, transparent) 0%, transparent 42%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 90%, transparent), color-mix(in srgb, var(--bg-secondary) 92%, transparent));
    box-shadow: inset 0 1px 0 color-mix(in srgb, #ffffff 32%, transparent);
  }

  .compact .hero-card {
    padding: 20px;
    gap: 18px;
  }

  .mic-section {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .mic-button {
    position: relative;
    width: 128px;
    height: 128px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
  }

  .mic-glow {
    position: absolute;
    width: 148px;
    height: 148px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--recording-glow) 0%, transparent 70%);
    transition: opacity 0.15s ease, transform 0.15s ease;
    pointer-events: none;
  }

  .mic-ring {
    width: 88px;
    height: 88px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 96%, transparent), color-mix(in srgb, var(--bg-secondary) 70%, transparent));
    border: 1px solid color-mix(in srgb, var(--border) 92%, transparent);
    color: var(--text-secondary);
    transition: all 0.2s ease;
    position: relative;
    z-index: 1;
    box-shadow: 0 16px 30px color-mix(in srgb, var(--shadow) 55%, transparent);
  }

  .mic-ring.recording {
    background: linear-gradient(180deg, color-mix(in srgb, var(--accent-hover) 82%, white 18%), var(--accent));
    border-color: var(--accent);
    color: #ffffff;
    box-shadow: 0 0 0 8px color-mix(in srgb, var(--accent) 12%, transparent), 0 18px 34px color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .mic-button:hover .mic-ring:not(.recording) {
    border-color: color-mix(in srgb, var(--accent) 38%, var(--border));
    color: var(--accent);
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 96%, transparent), color-mix(in srgb, var(--bg-tertiary) 72%, transparent));
  }

  .hero-title {
    font-size: 23px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .hero-text {
    font-size: 14px;
    line-height: 1.7;
    max-width: 280px;
  }

  .transcription-section {
    min-width: 0;
    min-height: 0;
  }

  .transcription-box {
    height: 100%;
    display: flex;
    flex-direction: column;
    padding: 24px;
    overflow: hidden;
    box-shadow: inset 0 1px 0 color-mix(in srgb, #ffffff 30%, transparent);
  }

  .transcription-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 18px;
    padding-bottom: 14px;
    border-bottom: 1px solid color-mix(in srgb, var(--border-light) 72%, transparent);
  }

  .transcription-text {
    flex: 1;
    font-size: 15px;
    line-height: 1.8;
    color: var(--text-primary);
    user-select: text;
    word-wrap: break-word;
    overflow-y: auto;
    padding-right: 8px;
  }

  .history-panel {
    min-height: 0;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    padding-right: 8px;
  }

  .history-item {
    padding: 18px;
    box-shadow: 0 10px 22px color-mix(in srgb, var(--shadow) 28%, transparent);
  }

  .history-item-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 10px;
  }

  .history-time {
    font-size: 15px;
    font-weight: 600;
    margin-bottom: 4px;
  }

  .history-mode {
    font-size: 13px;
    text-transform: capitalize;
  }

  .history-badge {
    padding: 6px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .history-badge.success {
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
  }

  .history-badge.failed {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    color: var(--danger);
  }

  .history-metrics {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    font-size: 13px;
  }

  .history-metrics span {
    padding: 6px 10px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--bg-primary) 82%, transparent);
    border: 1px solid color-mix(in srgb, var(--border-light) 85%, transparent);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 220px;
    padding: 28px;
    text-align: center;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, transparent), color-mix(in srgb, var(--bg-secondary) 86%, transparent));
  }

  .empty-inline {
    flex: 1;
    min-height: 0;
  }

  .empty-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary);
  }

  @media (max-width: 840px) {
    .app-container {
      flex-direction: column;
    }

    .sidebar {
      width: 100%;
      padding-top: 14px;
      border-right: none;
      border-bottom: 1px solid var(--border-light);
    }

    .sidebar-nav {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 4px;
    }

    .sidebar-section-label {
      display: none;
    }

    .sidebar-status-card {
      margin-top: 0;
    }

    .panel-header,
    .history-item-top {
      flex-direction: column;
    }

    .panel-summary {
      max-width: none;
      text-align: left;
    }

    .record-grid {
      grid-template-columns: 1fr;
    }

    .workspace {
      overflow: auto;
    }

    .panel {
      height: auto;
      min-height: 100%;
    }

    .transcription-box {
      min-height: 280px;
    }
  }
</style>
