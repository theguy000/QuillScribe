<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import TitleBar from './lib/TitleBar.svelte';
  import SettingsDialog from './lib/SettingsDialog.svelte';
  import Toast from './lib/Toast.svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import RecordPanel from './lib/RecordPanel.svelte';
  import TranscriptionPanel from './lib/TranscriptionPanel.svelte';
  import HistoryPanel from './lib/HistoryPanel.svelte';
  import { allThemeClasses } from './lib/themes.js';

  let isRecording = $state(false);
  let isTranscribing = $state(false);
  let transcriptionText = $state('');
  let statusMessage = $state('Ready');
  let currentTheme = $state(localStorage.getItem('qs-theme') || 'white');
  let customTitlebar = $state(true);
  let audioLevel = $state(0);
  let settings = $state(null);
  let activePanel = $state('record');
  let historyEntries = $state([]);
  let historyLoading = $state(false);
  let historyError = $state('');
  let toasts = $state([]);
  let showBurst = $state(false);
  let isCapturingShortcut = $state(false);

  let audioLevelInterval = null;

  const MAX_TOASTS = 3;
  const historyDays = 30;

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

  let isDark = $derived(
    currentTheme.startsWith('dark_') ||
    currentTheme === 'obsidian'
  );

  $effect(() => {
    document.documentElement.classList.remove('dark', ...allThemeClasses);
    if (isDark) {
      document.documentElement.classList.add('dark');
    }
    document.documentElement.classList.add(currentTheme);
    localStorage.setItem('qs-theme', currentTheme);
    if (settings) {
      invoke('set_tray_theme', { theme: currentTheme }).catch(() => {});
      invoke('set_taskbar_icon_theme', { theme: currentTheme }).catch(() => {});
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

  onMount(() => {
    /** @type {Array<() => void>} */
    let unlisteners = [];

    (async () => {
      await Promise.all([loadSettings(), loadHistory()]);

      unlisteners.push(
        await listen('hotkey-record-toggle', () => {
          if (isCapturingShortcut) return;
          if (isTranscribing) return;
          toggleRecording({ navigateToRecordOnStart: false });
        }),
        await listen('tray-open-settings', () => {
          activePanel = 'settings';
        }),
        await listen('tray-start-recording', () => {
          if (isCapturingShortcut) return;
          if (isTranscribing) return;
          if (!isRecording) {
            toggleRecording({ navigateToRecordOnStart: false });
          }
        }),
        await listen('tray-stop-recording', () => {
          if (isRecording) {
            toggleRecording({ navigateToRecordOnStart: false });
          }
        }),
        await listen('tray-model-changed', async () => {
          await loadSettings();
        }),
      );

      // Enable theme transitions only after the initial render is complete
      // to avoid first-paint jank from CSS transitions on every element.
      requestAnimationFrame(() => {
        document.body.classList.add('theme-ready');
      });
    })();

    return () => {
      for (const unlisten of unlisteners) {
        unlisten?.();
      }
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

    try {
      historyEntries = await invoke('get_recent_history', { days: historyDays });
    } catch (err) {
      historyEntries = [];
      historyError = `Failed to load history: ${err}`;
    } finally {
      historyLoading = false;
    }
  }

  async function toggleRecording(options = {}) {
    const { navigateToRecordOnStart = true } = options;

    if (isTranscribing) return;

    if (!isRecording) {
      try {
        if (navigateToRecordOnStart) {
          activePanel = 'record';
        }
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
      showBurst = true;
      setTimeout(() => { showBurst = false; }, 800);
    } catch (err) {
      statusMessage = `Error: ${err}`;
      showToast(`Transcription failed: ${err}`, 'error', 5000);
    } finally {
      isTranscribing = false;
    }
  }

  function handleNavigate(panel) {
    if (panel === 'history') {
      activePanel = 'history';
      loadHistory();
    } else {
      activePanel = panel;
    }
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
</script>

{#if customTitlebar}
  <TitleBar {isDark} />
{/if}

<main class="app-container">
  <Sidebar
    {activePanel}
    sessionCount={historyEntries.length}
    onnavigate={handleNavigate}
  />

  <section class="workspace">
    {#if activePanel === 'history'}
      <HistoryPanel
        {historyEntries}
        {historyLoading}
        {historyError}
        onrefresh={loadHistory}
      />
    {:else if activePanel === 'settings'}
      <SettingsDialog
        show={activePanel === 'settings'}
        embedded={true}
        onclose={() => { activePanel = 'record'; }}
        settings={settings}
        onsave={handleSaveSettings}
        onshortcutrecordingchange={(recordingShortcut) => {
          isCapturingShortcut = recordingShortcut;
        }}
      />
    {:else}
      <div class="record-grid">
        <RecordPanel
          {isRecording}
          {isTranscribing}
          {statusMessage}
          {audioLevel}
          {showBurst}
          ontogglerecording={toggleRecording}
        />
        <TranscriptionPanel bind:transcriptionText />
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

  .workspace {
    flex: 1;
    min-width: 0;
    padding: 22px;
    overflow: hidden;
  }

  .record-grid {
    height: 100%;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 18px;
    min-height: 0;
  }

  @media (max-width: 840px) {
    .app-container {
      flex-direction: column;
    }

    .record-grid {
      grid-template-columns: 1fr;
    }

    .workspace {
      overflow: auto;
    }
  }
</style>
