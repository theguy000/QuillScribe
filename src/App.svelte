<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import Toast from './lib/Toast.svelte';
  import { allThemeClasses } from './lib/themes.js';

  let isRecording = $state(false);
  let isTranscribing = $state(false);
  let transcriptionText = $state('');
  let isEditing = $state(false);
  let statusMessage = $state('Ready');
  let currentTheme = $state('white');
  let customTitlebar = $state(true);
  let audioLevel = $state(0);
  let settings = $state(null);
  let activePanel = $state('record');
  let historyEntries = $state([]);
  let historyLoading = $state(false);
  let historyError = $state('');
  let expandedHistoryIndex = $state(-1);
  let toasts = $state([]);

  let audioLevelInterval = null;

  const MAX_TOASTS = 3;

  function showToast(message, type = 'info', duration = 3000) {
    const id = Date.now() + Math.random();
    let updated = [...toasts, { id, message, type, duration }];
    if (updated.length > MAX_TOASTS) {
      updated = updated.slice(updated.length - MAX_TOASTS);
    }
    toasts = updated;
    setTimeout(() => {
      toasts = toasts.filter(t => t.id !== id);
    }, duration);
  }

  const historyDays = 30;

  let isDark = $derived(
    currentTheme.startsWith('dark_') ||
    currentTheme === 'obsidian'
  );

  let recentHistory = $derived([...historyEntries].reverse());

  $effect(() => {
    document.documentElement.classList.remove('dark', ...allThemeClasses);
    if (isDark) {
      document.documentElement.classList.add('dark');
    }
    document.documentElement.classList.add(currentTheme);
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

  onMount(() => {
    let unlisten;

    (async () => {
      await Promise.all([loadSettings(), loadHistory()]);
      unlisten = await listen('hotkey-record-toggle', () => {
        if (activePanel === 'settings') return;
        if (isTranscribing) return;
        toggleRecording();
      });
    })();

    return () => {
      unlisten?.();
    };
  });

  async function loadSettings() {
    try {
      const loadedSettings = await invoke('get_settings');
      if (loadedSettings) {
        settings = loadedSettings;
        currentTheme = loadedSettings.ui?.theme || 'white';
        customTitlebar = loadedSettings.ui?.custom_titlebar ?? true;
      }
    } catch (err) {
      console.warn('Failed to load settings:', err);
    }
  }

  async function loadHistory() {
    historyLoading = true;
    historyError = '';
    expandedHistoryIndex = -1;

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
    if (isTranscribing) return;

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
        showToast(`Failed to start recording: ${err}`, 'error', 5000);
      }
      return;
    }

    try {
      statusMessage = 'Transcribing...';
      isRecording = false;
      isTranscribing = true;
      const text = await invoke('stop_recording');
      transcriptionText = text || '';
      statusMessage = text ? 'Transcription complete' : 'No speech detected';
      if (!text) {
        showToast('No speech detected', 'warning');
      }
      await loadHistory();
    } catch (err) {
      statusMessage = `Error: ${err}`;
      showToast(`Transcription failed: ${err}`, 'error', 5000);
    } finally {
      isTranscribing = false;
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
      customTitlebar = newSettings.ui?.custom_titlebar ?? true;
      showToast('Settings saved successfully', 'success');
    } catch (err) {
      console.error('Failed to save settings:', err);
      showToast(`Failed to save settings: ${err}`, 'error', 5000);
    }
  }

  async function copyTranscription() {
    try {
      await navigator.clipboard.writeText(transcriptionText);
    } catch {
      // fallback: select all text
    }
  }

  function toggleEdit() {
    isEditing = !isEditing;
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

  function toggleHistoryExpand(index) {
    expandedHistoryIndex = expandedHistoryIndex === index ? -1 : index;
  }

  async function copyHistoryText(text) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // fallback silently
    }
  }
</script>

{#if customTitlebar}
  <TitleBar {isDark} />
{/if}

<main class="app-container">
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

    <div class="sidebar-user">
      <div class="user-avatar">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
          <circle cx="12" cy="7" r="4" />
        </svg>
      </div>
      <div class="user-info">
        <span class="user-name">User</span>
        <span class="user-meta">{historyEntries.length} sessions</span>
      </div>
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
            {#each recentHistory as entry, i}
              <div
                class="history-item"
                class:expanded={expandedHistoryIndex === i}
              >
                <button
                  class="history-item-toggle"
                  type="button"
                  onclick={() => toggleHistoryExpand(i)}
                >
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
                </button>

                {#if expandedHistoryIndex === i}
                  <div class="history-detail">
                    {#if entry.text}
                      <div class="history-detail-header">
                        <p class="history-detail-label">Transcription</p>
                        <button
                          class="action-btn"
                          type="button"
                          onclick={() => copyHistoryText(entry.text)}
                          title="Copy transcription"
                        >
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                          </svg>
                        </button>
                      </div>
                      <p class="history-detail-text">{entry.text}</p>
                    {:else}
                      <p class="history-detail-empty">No transcription text available for this session.</p>
                    {/if}
                  </div>
                {/if}
              </div>
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
      <div class="record-grid">
        <section class="record-card">
            <button
              class="mic-button"
              class:recording={isRecording}
              class:transcribing={isTranscribing}
              onclick={toggleRecording}
              disabled={isTranscribing}
              aria-label={isTranscribing ? 'Transcribing...' : isRecording ? 'Stop recording' : 'Start recording'}
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

          <div class="record-info">
            <p class="record-title">
              {#if isTranscribing}
                Transcribing audio
              {:else if isRecording}
                Recording in progress
              {:else if statusMessage.startsWith('Error:')}
                Something went wrong
              {:else}
                Ready to record
              {/if}
            </p>
            <p class="record-text">
              {#if isTranscribing}
                Processing your audio. This may take a moment depending on the length of the recording.
              {:else if isRecording}
                Click the microphone again to stop and send the captured audio for transcription.
              {:else if statusMessage.startsWith('Error:')}
                {statusMessage.slice(7)}
              {:else}
                Tap the microphone to start a fresh recording. The transcription result will appear in the panel beside it.
              {/if}
            </p>
          </div>
        </section>

        <section class="transcription-section">
          {#if transcriptionText}
            <div class="transcription-actions">
              <button class="action-btn" onclick={copyTranscription} title="Copy">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
              <button class="action-btn" class:active={isEditing} onclick={toggleEdit} title={isEditing ? 'Done editing' : 'Edit'}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                </svg>
              </button>
            </div>
            {#if isEditing}
              <textarea class="transcription-text transcription-edit" bind:value={transcriptionText}></textarea>
            {:else}
              <p class="transcription-text">{transcriptionText}</p>
            {/if}
          {:else}
            <div class="empty-inline">
              <p class="empty-title">No transcription yet</p>
              <p class="empty-copy">Once you stop a recording, the converted text will appear here.</p>
            </div>
          {/if}
        </section>
      </div>
    {/if}
  </section>
</main>

<Toast bind:toasts />

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
    margin: 22px 0 22px 22px;
    border: 1px solid color-mix(in srgb, var(--border-light) 92%, transparent);
    border-radius: 22px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 98%, transparent), color-mix(in srgb, var(--bg-secondary) 60%, transparent));
    box-shadow: 0 24px 60px color-mix(in srgb, var(--shadow-lg) 62%, transparent);
    flex-shrink: 0;
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

  .panel-kicker {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-muted);
  }

  .sidebar-tagline,
  .record-text,
  .empty-copy,
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

  .sidebar-user {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 8px;
    border-top: 1px solid color-mix(in srgb, var(--border-light) 60%, transparent);
  }

  .user-avatar {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-secondary));
    color: var(--accent);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .user-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .user-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.2;
  }

  .user-meta {
    font-size: 10px;
    color: var(--text-muted);
    line-height: 1.3;
  }

  .workspace {
    flex: 1;
    min-width: 0;
    padding: 22px;
    overflow: hidden;
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

  .panel-action {
    padding: 10px 14px;
    font-size: 13px;
    font-weight: 600;
  }

  .record-grid {
    height: 100%;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
    min-height: 0;
  }

  .record-card,
  .transcription-section,
  .history-item,
  .empty-state {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
  }

  .record-card {
    display: grid;
    grid-template-rows: 1fr 100px;
    align-items: center;
    justify-items: center;
    padding: 30px;
    text-align: center;
    overflow: visible;
    background:
      radial-gradient(circle at top, color-mix(in srgb, var(--accent) 18%, transparent) 0%, transparent 42%),
      linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 90%, transparent), color-mix(in srgb, var(--bg-secondary) 92%, transparent));
    box-shadow: inset 0 1px 0 var(--highlight);
  }


  .record-info {
    height: 100px;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    align-items: center;
    gap: 4px;
    overflow: hidden;
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
    color: var(--on-accent);
    box-shadow: 0 0 0 8px color-mix(in srgb, var(--accent) 12%, transparent), 0 18px 34px color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .mic-button.transcribing {
    cursor: wait;
    opacity: 0.6;
    pointer-events: none;
  }

  .mic-button:disabled {
    cursor: not-allowed;
  }

  .mic-button:hover .mic-ring:not(.recording) {
    border-color: color-mix(in srgb, var(--accent) 38%, var(--border));
    color: var(--accent);
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 96%, transparent), color-mix(in srgb, var(--bg-tertiary) 72%, transparent));
  }

  .record-title {
    font-size: 23px;
    font-weight: 700;
    letter-spacing: -0.02em;
  }

  .record-text {
    font-size: 14px;
    line-height: 1.7;
    max-width: 280px;
  }

  .transcription-section {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 20px;
    overflow: hidden;
  }

  .transcription-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-bottom: 12px;
    flex-shrink: 0;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: 1px solid var(--border-light);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
    color: var(--text-secondary);
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }

  .action-btn:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    color: var(--accent);
  }

  .action-btn.active {
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-primary));
    border-color: var(--accent);
    color: var(--accent);
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

  .transcription-edit {
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    font-family: inherit;
    padding: 0;
    padding-right: 8px;
    margin: 0;
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
    cursor: pointer;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .history-item:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border-light));
  }

  .history-item.expanded {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-light));
    box-shadow: 0 10px 22px color-mix(in srgb, var(--shadow) 28%, transparent),
                0 0 0 1px color-mix(in srgb, var(--accent) 15%, transparent);
  }

  .history-item-toggle {
    all: unset;
    display: block;
    width: 100%;
    cursor: pointer;
    text-align: left;
  }

  .history-item-toggle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
    border-radius: inherit;
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

  .history-detail {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid color-mix(in srgb, var(--border-light) 70%, transparent);
  }

  .history-detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  .history-detail-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .history-detail-text {
    font-size: 14px;
    line-height: 1.7;
    color: var(--text-primary);
    word-wrap: break-word;
    white-space: pre-wrap;
    max-height: 200px;
    overflow-y: auto;
    padding: 12px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--bg-primary) 90%, transparent);
    border: 1px solid color-mix(in srgb, var(--border-light) 70%, transparent);
    user-select: text;
  }

  .history-detail-empty {
    font-size: 13px;
    color: var(--text-muted);
    font-style: italic;
    text-align: center;
    padding: 12px;
  }

  .empty-state,
  .empty-inline {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
  }

  .empty-state {
    min-height: 220px;
    padding: 28px;
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
      width: auto;
      margin: 14px 14px 0 14px;
      padding-top: 14px;
      border-right: none;
      border-bottom: none;
    }

    .sidebar-nav {
      display: grid;
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 4px;
    }

    .sidebar-section-label {
      display: none;
    }

    .sidebar-user {
      margin-top: 0;
      border-top: none;
      padding-top: 4px;
    }

    .panel-header,
    .history-item-top {
      flex-direction: column;
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

  }
</style>
