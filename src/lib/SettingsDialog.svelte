<script>
  import { invoke } from '@tauri-apps/api/core'
  import { getVersion } from '@tauri-apps/api/app'
  import { onDestroy } from 'svelte'

  let {
    show,
    onclose,
    settings,
    onsave,
    embedded = false,
    noCompositor = false,
    updateAvailable = null,
    updateDownloading = false,
    updateProgress = 0,
    updateChecking = false,
    oninstallupdate = () => {},
    oncheckupdate = () => {},
    onshortcutrecordingchange = () => {},
  } = $props()

  let activeTab = $state('audio')
  let localSettings = $state(null)
  let audioDevices = $state([])
  let languages = $state([])
  let localModels = $state([])
  let modelInfo = $state(null)
  let showApiKey = $state(false)
  let modelCategory = $state('All')
  let testingMic = $state(false)
  let micTestLevel = $state(0)
  let micTestInterval = $state(null)
  let loadingDevices = $state(false)
  let downloadingModel = $state(false)
  let downloadError = $state(null)
  let downloadedModels = $state([])
  let deletingModel = $state(false)
  let recordingShortcut = $state(false)
  let shortcutInputEl = $state(null)

  let appVersion = $state('')

  // Linux paste tool status (only relevant for typing modes)
  let isLinuxPlatform = $state(false)
  let pasteToolStatus = $state(null)

  const tabs = [
    { id: 'audio', label: 'Audio' },
    { id: 'whisper', label: 'Whisper' },
    { id: 'ui', label: 'UI' },
    { id: 'output', label: 'Output' },
    { id: 'keyboard', label: 'Keyboard' },
    { id: 'about', label: 'About' },
  ]

  import { themes } from './themes.js'
  import { overlayStyles, styleSupportsOpacity } from './overlayStyles.js'

  const outputModes = [
    { value: 0, label: 'Copy to Clipboard', description: 'Copies text to clipboard only' },
    { value: 1, label: 'Type to Active Window', description: 'Types text into the focused app without affecting clipboard' },
    { value: 2, label: 'Copy & Type to Window', description: 'Copies to clipboard and types into the focused app' },
    { value: 3, label: 'Display Only', description: 'Shows the result in the app only' },
  ]

  const apiModels = ['gpt-4o-transcribe', 'gpt-4o-mini-transcribe']

  const modelCategories = ['All', 'Tiny', 'Base', 'Small', 'Medium', 'Large', 'Distilled']

  let filteredModels = $derived(
    modelCategory === 'All'
      ? localModels
      : localModels.filter(m => {
          const name = (m.name || m).toLowerCase()
          return name.includes(modelCategory.toLowerCase())
        })
  )

  /** Strip markdown syntax for plain-text display of release notes. */
  function stripMarkdown(text) {
    return text
      .replace(/^#{1,6}\s+/gm, '')       // Remove heading markers
      .replace(/\*\*(.+?)\*\*/g, '$1')   // Bold **text** → text
      .replace(/\*(.+?)\*/g, '$1')       // Italic *text* → text
      .replace(/`(.+?)`/g, '$1')         // Inline code `text` → text
      .replace(/^\s*[-*+]\s+/gm, '• ')   // List items → bullet
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1') // Links [text](url) → text
      .trim()
  }

  onDestroy(() => {
    onshortcutrecordingchange(false)
  })

  $effect(() => {
    if (show && settings) {
      localSettings = JSON.parse(JSON.stringify(settings))
      loadInitialData()
    }
    return () => {
      if (testingMic) stopMicTest()
    }
  })

  async function loadInitialData() {
    await Promise.all([
      loadAudioDevices(),
      loadLanguages(),
      loadLocalModels(),
      loadDownloadedModels(),
      loadPasteToolStatus(),
      getVersion().then(v => appVersion = v).catch(() => {}),
    ])
  }

  async function loadAudioDevices() {
    loadingDevices = true
    try {
      audioDevices = await invoke('get_audio_devices')
    } catch (e) {
      console.error('Failed to load audio devices:', e)
      audioDevices = []
    } finally {
      loadingDevices = false
    }
  }

  async function loadLanguages() {
    try {
      const raw = await invoke('get_available_languages')
      // Normalize: backend returns [code, name] tuples; ensure we always have { code, name } objects.
      languages = (raw || []).map(lang =>
        Array.isArray(lang) ? { code: lang[0], name: lang[1] } : lang
      )
    } catch (e) {
      console.error('Failed to load languages:', e)
      languages = []
    }
  }

  async function loadLocalModels() {
    try {
      localModels = await invoke('get_available_local_models')
    } catch (e) {
      console.error('Failed to load local models:', e)
      localModels = []
    }
  }

  async function loadModelInfo(modelName) {
    try {
      modelInfo = await invoke('get_model_info', { modelName: modelName })
    } catch (e) {
      console.error('Failed to load model info:', e)
      modelInfo = null
    }
  }

  async function loadDownloadedModels() {
    try {
      downloadedModels = await invoke('get_downloaded_models')
    } catch (e) {
      console.error('Failed to load downloaded models:', e)
      downloadedModels = []
    }
  }

  async function loadPasteToolStatus() {
    try {
      isLinuxPlatform = await invoke('is_linux')
      if (isLinuxPlatform) {
        pasteToolStatus = await invoke('check_paste_tool_status')
      }
    } catch (e) {
      console.error('Failed to check paste tool status:', e)
    }
  }

  /** Whether the currently selected output mode requires typing (paste simulation). */
  let needsTyping = $derived(
    localSettings?.output?.mode === 1 || localSettings?.output?.mode === 2
  )

  /** Whether no paste tool is available on Linux when a typing mode is selected. */
  let showPasteToolWarning = $derived(
    isLinuxPlatform && needsTyping && pasteToolStatus && pasteToolStatus.detected_tool === 'None'
  )

  async function handleDownloadModel() {
    const model = localSettings.whisper.local_model
    downloadingModel = true
    downloadError = null
    try {
      await invoke('download_model', { modelName: model })
      await loadDownloadedModels()
    } catch (e) {
      console.error('Failed to download model:', e)
      downloadError = String(e)
    } finally {
      downloadingModel = false
    }
  }

  async function handleDeleteModel() {
    const model = localSettings.whisper.local_model
    deletingModel = true
    try {
      await invoke('delete_model', { modelName: model })
      await loadDownloadedModels()
    } catch (e) {
      console.error('Failed to delete model:', e)
    } finally {
      deletingModel = false
    }
  }

  async function startMicTest() {
    testingMic = true
    micTestLevel = 0
    try {
      await invoke('start_mic_test', { deviceId: localSettings.audio.device_id ?? null })
      // Poll audio level every 50ms
      micTestInterval = setInterval(async () => {
        try {
          micTestLevel = await invoke('get_audio_level')
        } catch (e) {
          console.error('Failed to get audio level:', e)
        }
      }, 50)
    } catch (e) {
      console.error('Mic test failed:', e)
      testingMic = false
    }
  }

  async function stopMicTest() {
    if (micTestInterval) {
      clearInterval(micTestInterval)
      micTestInterval = null
    }
    try {
      await invoke('stop_mic_test')
    } catch (e) {
      console.error('Failed to stop mic test:', e)
    }
    testingMic = false
    micTestLevel = 0
  }

  function handleSave() {
    if (recordingShortcut) stopRecordingShortcut()
    if (testingMic) stopMicTest()
    onsave(localSettings)
  }

  function handleCancel() {
    if (recordingShortcut) stopRecordingShortcut()
    if (testingMic) stopMicTest()
    onclose()
  }

  function handleOverlayClick(e) {
    if (!embedded && e.target === e.currentTarget) {
      if (testingMic) stopMicTest()
      onclose()
    }
  }

  function handleKeydown(e) {
    if (recordingShortcut) return
    if (e.key === 'Escape') {
      if (testingMic) stopMicTest()
      onclose()
    }
  }

  function handleLocalModelChange(value) {
    localSettings.whisper.local_model = value
    loadModelInfo(value)
  }

  function getFieldValue(event) {
    return /** @type {HTMLInputElement | HTMLSelectElement} */ (event.currentTarget).value
  }

  function getCheckedValue(event) {
    return /** @type {HTMLInputElement} */ (event.currentTarget).checked
  }

  function startRecordingShortcut() {
    recordingShortcut = true
    onshortcutrecordingchange(true)
    // Focus the input on next tick so it receives key events
    requestAnimationFrame(() => {
      shortcutInputEl?.focus()
    })
  }

  function stopRecordingShortcut() {
    recordingShortcut = false
    onshortcutrecordingchange(false)
  }

  function handleShortcutKeydown(e) {
    if (!recordingShortcut) return

    e.preventDefault()
    e.stopPropagation()

    // Escape cancels recording
    if (e.key === 'Escape') {
      stopRecordingShortcut()
      return
    }

    // Ignore standalone modifier presses — wait for a non-modifier key
    const modifierKeys = ['Control', 'Shift', 'Alt', 'Meta']
    if (modifierKeys.includes(e.key)) return

    const parts = []
    if (e.ctrlKey) parts.push('Ctrl')
    if (e.altKey) parts.push('Alt')
    if (e.shiftKey) parts.push('Shift')
    if (e.metaKey) parts.push('Meta')

    // Require at least one modifier to avoid single-key global shortcuts
    // that would conflict with normal typing across the OS.
    if (parts.length === 0) return

    // Normalise the key name
    let key = e.key
    if (key === ' ') key = 'Space'
    else if (key.length === 1) key = key.toUpperCase()

    parts.push(key)

    localSettings.shortcuts.record_toggle = parts.join('+')
    stopRecordingShortcut()
  }
</script>

{#if show && localSettings}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" class:embedded onclick={handleOverlayClick} onkeydown={handleKeydown}>
    <div class="settings-card" class:embedded>
      <div class="settings-header">
        <h2>Settings</h2>
        {#if !embedded}
          <button class="close-btn" onclick={onclose} aria-label="Close">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M1 1L13 13M1 13L13 1" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
            </svg>
          </button>
        {/if}
      </div>

      <nav class="tabs">
        {#each tabs as tab}
          <button
            class="tab"
            class:active={activeTab === tab.id}
            onclick={() => activeTab = tab.id}
          >
            {tab.label}
            {#if tab.id === 'about' && updateAvailable}
              <span class="tab-dot"></span>
            {/if}
          </button>
        {/each}
      </nav>

      <div class="tab-content">
        <!-- Audio Tab -->
        {#if activeTab === 'audio'}
          <div class="tab-panel">
            <div class="field">
              <span class="field-label">Microphone</span>
              <div class="field-row">
                <select
                  class="field-select"
                  value={localSettings.audio.device_id}
                  onchange={(e) => localSettings.audio.device_id = getFieldValue(e) || null}
                >
                  <option value="">Default</option>
                  {#each audioDevices as device}
                    <option value={device.id}>{device.name}</option>
                  {/each}
                </select>
                <button class="icon-btn" onclick={loadAudioDevices} disabled={loadingDevices} title="Refresh devices">
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none" class:spinning={loadingDevices}>
                    <path d="M14 8A6 6 0 1 1 8 2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                    <path d="M8 0L10 2L8 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                  </svg>
                </button>
              </div>
            </div>

            <div class="field">
              <span class="field-label">Test Microphone</span>
              <div class="mic-test-container">
                <button
                  class="mic-test-toggle"
                  class:active={testingMic}
                  onclick={() => testingMic ? stopMicTest() : startMicTest()}
                >
                  {testingMic ? 'Stop' : 'Test'}
                </button>
                <div class="mic-pill-track">
                  <div
                    class="mic-pill-fill"
                    class:loud={testingMic && micTestLevel > 0.15}
                    style:width="{testingMic ? Math.min(micTestLevel / 0.25 * 100, 100) : 0}%"
                  ></div>
                </div>
              </div>
              {#if testingMic}
                <p class="field-hint mic-hint">Speak into your mic — the pill should glow.</p>
              {/if}
            </div>


            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.audio.sounds_enabled}
                  onchange={(e) => localSettings.audio.sounds_enabled = getCheckedValue(e)}
                />
                <span>Enable sounds</span>
              </label>
            </div>

            <div class="field">
              <span class="field-label">Microphone Blocklist</span>
              <button class="action-btn" onclick={() => alert('Microphone blocklist management coming soon!')}>
                Manage Blocklist
              </button>
              <p class="field-hint">Manage ignored microphone devices.</p>
            </div>
          </div>
        {/if}

        <!-- Whisper Tab -->
        {#if activeTab === 'whisper'}
          <div class="tab-panel">
            <div class="field">
              <span class="field-label">Mode</span>
              <div class="radio-group">
                <label class="radio-label">
                  <input
                    type="radio"
                    name="whisper-mode"
                    value="api"
                    checked={localSettings.whisper.mode === 'api'}
                    onchange={() => localSettings.whisper.mode = 'api'}
                  />
                  <span>OpenAI API</span>
                </label>
                <label class="radio-label">
                  <input
                    type="radio"
                    name="whisper-mode"
                    value="local"
                    checked={localSettings.whisper.mode === 'local'}
                    onchange={() => localSettings.whisper.mode = 'local'}
                  />
                  <span>Local Model</span>
                </label>
              </div>
            </div>

            {#if localSettings.whisper.mode === 'api'}
              <div class="field">
                <span class="field-label">API Key</span>
                <div class="field-row">
                  <input
                    class="field-input"
                    type={showApiKey ? 'text' : 'password'}
                    value={localSettings.whisper.api_key}
                    oninput={(e) => localSettings.whisper.api_key = getFieldValue(e)}
                    placeholder="sk-..."
                  />
                  <button class="icon-btn" onclick={() => showApiKey = !showApiKey} title={showApiKey ? 'Hide' : 'Show'}>
                    {#if showApiKey}
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                        <path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5z" stroke="currentColor" stroke-width="1.5"/>
                        <circle cx="8" cy="8" r="2" stroke="currentColor" stroke-width="1.5"/>
                      </svg>
                    {:else}
                      <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                        <path d="M1 8s2.5-5 7-5 7 5 7 5-2.5 5-7 5-7-5-7-5z" stroke="currentColor" stroke-width="1.5"/>
                        <line x1="2" y1="2" x2="14" y2="14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                      </svg>
                    {/if}
                  </button>
                </div>
              </div>

              <div class="field">
                <span class="field-label">API Model</span>
                <select
                  class="field-select"
                  value={localSettings.whisper.api_model}
                  onchange={(e) => localSettings.whisper.api_model = getFieldValue(e)}
                >
                  {#each apiModels as model}
                    <option value={model}>{model}</option>
                  {/each}
                </select>
              </div>

              <div class="field">
                <span class="field-label">Language</span>
                <select
                  class="field-select"
                  value={localSettings.whisper.api_language}
                  onchange={(e) => localSettings.whisper.api_language = getFieldValue(e)}
                >
                  {#each languages as lang}
                    <option value={lang.code}>{lang.name}</option>
                  {/each}
                </select>
              </div>
            {:else}
              <div class="field">
                <span class="field-label">Category</span>
                <select
                  class="field-select"
                  value={modelCategory}
                  onchange={(e) => modelCategory = getFieldValue(e)}
                >
                  {#each modelCategories as cat}
                    <option value={cat}>{cat}</option>
                  {/each}
                </select>
              </div>

              <div class="field">
                <span class="field-label">Model</span>
                <select
                  class="field-select"
                  value={localSettings.whisper.local_model}
                  onchange={(e) => handleLocalModelChange(getFieldValue(e))}
                >
                  {#each filteredModels as model}
                    <option value={model.id || model}>{model.name || model}</option>
                  {/each}
                </select>
              </div>

              <div class="field">
                <div class="model-status-row">
                  {#if downloadedModels.includes(localSettings.whisper.local_model)}
                    <span class="badge badge-success">Downloaded</span>
                    <button
                      class="action-btn btn-danger-outline btn-sm"
                      onclick={handleDeleteModel}
                      disabled={deletingModel}
                    >
                      {deletingModel ? 'Deleting...' : 'Delete Model'}
                    </button>
                  {:else}
                    <span class="badge badge-warning">Not Downloaded</span>
                    <button
                      class="action-btn btn-sm"
                      onclick={handleDownloadModel}
                      disabled={downloadingModel}
                    >
                      {downloadingModel ? 'Downloading...' : 'Download Model'}
                    </button>
                  {/if}
                </div>
                {#if downloadingModel}
                  <div class="download-progress">
                    <div class="progress-bar">
                      <div class="progress-bar-indeterminate"></div>
                    </div>
                    <p class="field-hint">Downloading model — this may take a while for larger models.</p>
                  </div>
                {/if}
                {#if downloadError}
                  <p class="field-error">{downloadError}</p>
                {/if}
              </div>

              <div class="field">
                <span class="field-label">Language</span>
                <select
                  class="field-select"
                  value={localSettings.whisper.api_language}
                  onchange={(e) => localSettings.whisper.api_language = getFieldValue(e)}
                >
                  <option value="auto">Auto-detect</option>
                  {#each languages as lang}
                    <option value={lang.code}>{lang.name}</option>
                  {/each}
                </select>
              </div>

              {#if modelInfo}
                <div class="model-info">
                  <div class="model-info-row">
                    <span class="info-label">Size</span>
                    <span class="info-value">{modelInfo.size || 'N/A'}</span>
                  </div>
                  <div class="model-info-row">
                    <span class="info-label">Memory</span>
                    <span class="info-value">{modelInfo.memory || 'N/A'}</span>
                  </div>
                  <div class="model-info-row">
                    <span class="info-label">Speed</span>
                    <span class="info-value">{modelInfo.speed || 'N/A'}</span>
                  </div>
                  <div class="model-info-row">
                    <span class="info-label">Quality</span>
                    <span class="info-value">{modelInfo.quality || 'N/A'}</span>
                  </div>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- UI Tab -->
        {#if activeTab === 'ui'}
          <div class="tab-panel">
            <div class="field">
              <span class="field-label">Theme</span>
              <select
                class="field-select"
                value={localSettings.ui.theme}
                onchange={(e) => localSettings.ui.theme = getFieldValue(e)}
              >
                {#each themes as theme}
                  <option value={theme.value}>{theme.label}</option>
                {/each}
              </select>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.custom_titlebar}
                  onchange={(e) => localSettings.ui.custom_titlebar = getCheckedValue(e)}
                />
                <span>Custom titlebar</span>
              </label>
            </div>


            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.always_on_top}
                  onchange={(e) => localSettings.ui.always_on_top = getCheckedValue(e)}
                />
                <span>Always on top</span>
              </label>
            </div>

            <div class="field">
              <span class="field-label">Recording Overlay Style</span>
              <select
                class="field-select"
                value={localSettings.ui.overlay_mode ?? 'minimal'}
                onchange={(e) => localSettings.ui.overlay_mode = getFieldValue(e)}
              >
                <option value="minimal">Minimal (bars only)</option>
                <option value="full">Full (bars, timer, stop button)</option>
              </select>
              <p class="field-hint">Style of the floating overlay shown when recording and the app is unfocused.</p>
            </div>

            <div class="field">
              <span class="field-label">Overlay Visual Style</span>
              <select
                class="field-select"
                value={localSettings.ui.overlay_style ?? 'default'}
                onchange={(e) => localSettings.ui.overlay_style = getFieldValue(e)}
              >
                {#each overlayStyles as style}
                  <option value={style.value}>{style.label}</option>
                {/each}
              </select>
              <p class="field-hint">Visual appearance of the recording overlay (background, border, shadows).</p>
            </div>

            {#if styleSupportsOpacity(localSettings.ui.overlay_style ?? 'default')}
              <div class="field">
                <span class="field-label">Overlay Opacity — {Math.round((localSettings.ui.overlay_opacity ?? 0.85) * 100)}%</span>
                <input
                  type="range"
                  class="field-range"
                  min="0.3"
                  max="1"
                  step="0.05"
                  value={localSettings.ui.overlay_opacity ?? 0.85}
                  oninput={(e) => localSettings.ui.overlay_opacity = parseFloat(getFieldValue(e))}
                />
                <p class="field-hint">Controls the background transparency of the glass overlay. Lower values are more transparent.</p>
              </div>
            {/if}

            {#if noCompositor}
              <p class="field-hint" style="margin-top: 4px;">Your system does not have a window compositor, so the overlay will display as a rectangle instead of a pill shape. Installing a compositor (e.g. picom, compton) will enable the pill style.</p>
            {/if}

            <div class="field">
              <span class="field-label">Maximum Recent History</span>
              <div class="stepper">
                <button
                  class="stepper-btn"
                  onclick={() => localSettings.advanced.max_history_entries = Math.max(1, localSettings.advanced.max_history_entries - 10)}
                  disabled={localSettings.advanced.max_history_entries <= 1}
                  aria-label="Decrease"
                >
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                    <path d="M2.5 6H9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                  </svg>
                </button>
                <input
                  class="stepper-value"
                  type="text"
                  inputmode="numeric"
                  value={localSettings.advanced.max_history_entries}
                  onchange={(e) => {
                    const v = parseInt(getFieldValue(e), 10)
                    localSettings.advanced.max_history_entries = isNaN(v) ? 100 : Math.max(1, Math.min(1000, v))
                  }}
                />
                <button
                  class="stepper-btn"
                  onclick={() => localSettings.advanced.max_history_entries = Math.min(1000, localSettings.advanced.max_history_entries + 10)}
                  disabled={localSettings.advanced.max_history_entries >= 1000}
                  aria-label="Increase"
                >
                  <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                    <path d="M6 2.5V9.5M2.5 6H9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                  </svg>
                </button>
              </div>
              <p class="field-hint">Maximum number of transcription history entries to keep (1–1000).</p>
            </div>


          </div>
        {/if}

        <!-- Output Tab -->
        {#if activeTab === 'output'}
          <div class="tab-panel">
            <div class="field">
              <span class="field-label">Output Mode</span>
              <div class="radio-group">
                {#each outputModes as mode}
                  <label class="radio-label">
                    <input
                      type="radio"
                      name="output-mode"
                      value={mode.value}
                      checked={localSettings.output.mode === mode.value}
                      onchange={() => localSettings.output.mode = mode.value}
                    />
                    <span class="radio-text">
                      <span>{mode.label}</span>
                      <span class="radio-description">{mode.description}</span>
                    </span>
                  </label>
                {/each}
              </div>
            </div>

            {#if showPasteToolWarning}
              <div class="paste-tool-warning">
                <div class="paste-tool-warning-header">
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <path d="M8 1.5L1 14h14L8 1.5z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" fill="none"/>
                    <path d="M8 6v3.5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
                    <circle cx="8" cy="11.5" r="0.7" fill="currentColor"/>
                  </svg>
                  <span>Paste tool required for typing modes on Linux</span>
                </div>
                <pre class="paste-tool-hint">{pasteToolStatus.setup_hint}</pre>
              </div>
            {/if}

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.output.silent_mode}
                  onchange={(e) => localSettings.output.silent_mode = getCheckedValue(e)}
                />
                <span>Silent mode</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.output.auto_clear}
                  onchange={(e) => localSettings.output.auto_clear = getCheckedValue(e)}
                />
                <span>Auto-clear clipboard</span>
              </label>
            </div>

            {#if localSettings.output.auto_clear}
              <div class="field">
                <span class="field-label">Auto-clear delay (seconds)</span>
                <div class="stepper">
                  <button
                    class="stepper-btn"
                    onclick={() => localSettings.output.auto_clear_delay = Math.max(1, localSettings.output.auto_clear_delay - 1)}
                    disabled={localSettings.output.auto_clear_delay <= 1}
                    aria-label="Decrease"
                  >
                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                      <path d="M2.5 6H9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                    </svg>
                  </button>
                  <input
                    class="stepper-value"
                    type="text"
                    inputmode="numeric"
                    value={localSettings.output.auto_clear_delay}
                    onchange={(e) => {
                      const v = parseInt(getFieldValue(e), 10)
                      localSettings.output.auto_clear_delay = isNaN(v) ? 5 : Math.max(1, Math.min(300, v))
                    }}
                  />
                  <button
                    class="stepper-btn"
                    onclick={() => localSettings.output.auto_clear_delay = Math.min(300, localSettings.output.auto_clear_delay + 1)}
                    disabled={localSettings.output.auto_clear_delay >= 300}
                    aria-label="Increase"
                  >
                    <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                      <path d="M6 2.5V9.5M2.5 6H9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                    </svg>
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/if}

        <!-- Keyboard Tab -->
        {#if activeTab === 'keyboard'}
          <div class="tab-panel">
            <div class="field">
              <span class="field-label">Record Toggle Shortcut</span>
              <div class="field-row">
                <input
                  bind:this={shortcutInputEl}
                  class="field-input shortcut-input"
                  class:recording-shortcut={recordingShortcut}
                  type="text"
                  value={recordingShortcut ? 'Press a key combination...' : localSettings.shortcuts.record_toggle}
                  readonly
                  onkeydown={handleShortcutKeydown}
                  placeholder="e.g. Meta+Shift+`"
                />
                {#if recordingShortcut}
                  <button class="action-btn shortcut-cancel-btn" onclick={stopRecordingShortcut}>
                    Cancel
                  </button>
                {:else}
                  <button class="action-btn" onclick={startRecordingShortcut}>
                    Record
                  </button>
                {/if}
              </div>
              <p class="field-hint">Click <strong>Record</strong>, then press the key combination you want to use.</p>
            </div>
          </div>
        {/if}


        <!-- About Tab -->
        {#if activeTab === 'about'}
          <div class="tab-panel about-panel">
            <div class="about-header">
              <div>
                <h3 class="about-title">QuillScribe</h3>
                <p class="about-version">{appVersion ? `v${appVersion}` : ''}</p>
              </div>
            </div>

            <p class="about-description">
              Beautiful voice-to-text transcription app. Record your voice, get text instantly using OpenAI's Whisper API or a local model.
            </p>

            <!-- Update Section -->
            <div class="field">
              <span class="field-label">Updates</span>
              {#if updateAvailable}
                <div class="update-card">
                  <div class="update-card-header">
                    <span class="update-badge">New version available</span>
                    <span class="update-version">v{updateAvailable.version}</span>
                  </div>
                  {#if updateAvailable.body}
                    <p class="update-notes">{stripMarkdown(updateAvailable.body)}</p>
                  {/if}
                  {#if updateDownloading}
                    <div class="update-progress-wrap">
                      <div class="update-progress-bar">
                        <div class="update-progress-fill" style="width: {updateProgress}%"></div>
                      </div>
                      <span class="update-progress-text">{updateProgress}%</span>
                    </div>
                  {:else}
                    <button class="action-btn update-btn" onclick={() => oninstallupdate()}>
                      Download & Install
                    </button>
                  {/if}
                </div>
              {:else}
                <div class="update-no-update-row">
                  <p class="about-up-to-date">You're on the latest version.</p>
                  <button class="action-btn btn-sm" onclick={() => oncheckupdate()} disabled={updateChecking}>
                    {updateChecking ? 'Checking...' : 'Check for updates'}
                  </button>
                </div>
              {/if}
            </div>

            <div class="about-divider"></div>

            <div class="field">
              <span class="field-label">Project</span>
              <div class="about-info-grid">
                <div class="about-info-item">
                  <span class="about-info-label">License</span>
                  <span class="about-info-value">MIT</span>
                </div>
                <div class="about-info-item">
                  <span class="about-info-label">Framework</span>
                  <span class="about-info-value">Tauri v2 + Svelte 5</span>
                </div>
                <div class="about-info-item">
                  <span class="about-info-label">Backend</span>
                  <span class="about-info-value">Rust</span>
                </div>
              </div>
            </div>

            <div class="field">
              <span class="field-label">Links</span>
              <div class="about-link-row">
                <a href="https://github.com/theguy000/QuillScribe" target="_blank" rel="noopener noreferrer" class="about-link">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
                  </svg>
                  GitHub
                </a>
                <a href="https://github.com/theguy000/QuillScribe/issues" target="_blank" rel="noopener noreferrer" class="about-link">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10" />
                    <line x1="12" y1="8" x2="12" y2="12" />
                    <line x1="12" y1="16" x2="12.01" y2="16" />
                  </svg>
                  Report Issue
                </a>
              </div>
            </div>
          </div>
        {/if}

      </div>

      <div class="settings-footer">
        <button class="btn btn-secondary" onclick={handleCancel}>Cancel</button>
        <button class="btn btn-primary" onclick={handleSave}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
  }

  .settings-overlay.embedded {
    position: static;
    inset: auto;
    z-index: auto;
    height: 100%;
    align-items: stretch;
    justify-content: stretch;
    background: transparent;
    backdrop-filter: none;
  }

  .settings-card {
    width: 92%;
    max-width: 480px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 8px 32px var(--shadow-lg);
    overflow: hidden;
  }

  .settings-card.embedded {
    width: 100%;
    max-width: none;
    max-height: none;
    height: 100%;
    border-radius: 22px;
    border: 1px solid color-mix(in srgb, var(--border-light) 92%, transparent);
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 98%, transparent), color-mix(in srgb, var(--bg-secondary) 60%, transparent));
    box-shadow: 0 24px 60px color-mix(in srgb, var(--shadow-lg) 62%, transparent);
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px 12px;
    border-bottom: 1px solid var(--border-light);
  }

  .settings-card.embedded .settings-header {
    padding: 24px 26px 18px;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .settings-card.embedded .settings-header h2 {
    font-size: 30px;
    line-height: 1.1;
    letter-spacing: -0.03em;
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .close-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  /* Tabs */
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border-light);
    overflow-x: auto;
  }

  .settings-card.embedded .tabs {
    padding: 0 26px;
  }

  .tab {
    padding: 10px 14px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--text-secondary);
    border-bottom: 2px solid transparent;
    cursor: pointer;
    white-space: nowrap;
    transition: color 0.15s, border-color 0.15s;
  }

  .tab:hover {
    color: var(--text-primary);
  }

  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  /* Tab content */
  .tab-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .settings-card.embedded .tab-content {
    padding-bottom: 8px;
  }

  .tab-panel {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .settings-card.embedded .tab-panel {
    padding: 22px 26px;
    gap: 16px;
  }

  /* Fields */
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field-label {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .field-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .field-select,
  .field-input {
    width: 100%;
    padding: 8px 12px;
    font-size: 13px;
    color: var(--text-primary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    outline: none;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .field-select:focus,
  .field-input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }

  /* Custom stepper */
  .stepper {
    display: inline-flex;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-secondary);
    align-self: flex-start;
  }

  .stepper-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    color: var(--text-secondary);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .stepper-btn:hover:not(:disabled) {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .stepper-btn:active:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    color: var(--accent);
  }

  .stepper-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .stepper-value {
    width: 52px;
    text-align: center;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    background: transparent;
    border: none;
    border-left: 1px solid var(--border-light);
    border-right: 1px solid var(--border-light);
    outline: none;
    padding: 6px 0;
    -moz-appearance: textfield;
  }

  .stepper-value:focus {
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .field-hint {
    font-size: 11.5px;
    color: var(--text-muted);
    margin: 0;
  }

  /* Shortcut recording */
  .shortcut-input {
    cursor: default;
  }

  .shortcut-input.recording-shortcut {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-glow);
    color: var(--text-muted);
    font-style: italic;
  }

  .shortcut-cancel-btn {
    color: var(--text-secondary);
    border-color: var(--border);
  }

  .shortcut-cancel-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--border);
  }

  /* Checkbox */
  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
  }

  .checkbox-label input[type="checkbox"] {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    margin: 0;
    cursor: pointer;
    flex-shrink: 0;
    display: grid;
    place-content: center;
    background: var(--bg-secondary);
    border: 1.5px solid color-mix(in srgb, var(--accent) 45%, transparent);
    border-radius: 4px;
    box-sizing: border-box;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .checkbox-label input[type="checkbox"]::before {
    content: '';
    width: 8px;
    height: 4px;
    border-left: 2px solid var(--on-accent);
    border-bottom: 2px solid var(--on-accent);
    transform: rotate(-45deg) scale(0);
    transform-origin: center;
    transition: transform 0.15s ease;
  }

  .checkbox-label input[type="checkbox"]:checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  .checkbox-label input[type="checkbox"]:checked::before {
    transform: rotate(-45deg) scale(1);
  }

  .checkbox-label input[type="checkbox"]:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }

  /* Radio */
  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .radio-label {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
  }

  .radio-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .radio-description {
    font-size: 11px;
    color: var(--text-secondary);
    opacity: 0.7;
  }

  .paste-tool-warning {
    background: color-mix(in srgb, var(--warning, #f0ad4e) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning, #f0ad4e) 35%, transparent);
    border-radius: 8px;
    padding: 10px 12px;
    margin-bottom: 4px;
  }

  .paste-tool-warning-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--warning, #f0ad4e);
    margin-bottom: 6px;
  }

  .paste-tool-hint {
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-secondary);
    white-space: pre-wrap;
    margin: 0;
    font-family: 'SF Mono', 'Menlo', 'Consolas', monospace;
  }

  .radio-label input[type="radio"] {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    margin: 2px 0 0 0;
    cursor: pointer;
    flex-shrink: 0;
    border-radius: 50%;
    display: grid;
    place-content: center;
    background: var(--bg-secondary);
    border: 1.5px solid color-mix(in srgb, var(--accent) 45%, transparent);
    box-sizing: border-box;
    transition: border-color 0.15s ease;
  }

  .radio-label input[type="radio"]::before {
    content: '';
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    transform: scale(0);
    transition: transform 0.15s ease;
  }

  .radio-label input[type="radio"]:checked {
    border-color: var(--accent);
  }

  .radio-label input[type="radio"]:checked::before {
    transform: scale(1);
  }

  .radio-label input[type="radio"]:focus-visible {
    outline: 2px solid color-mix(in srgb, var(--accent) 50%, transparent);
    outline-offset: 1px;
  }

  /* Icon button */
  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    border-radius: 8px;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .icon-btn:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--accent);
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Action button */
  .action-btn {
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 500;
    color: var(--accent);
    background: transparent;
    border: 1px solid var(--accent);
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    align-self: flex-start;
  }

  .action-btn:hover {
    background: var(--accent);
    color: var(--on-accent);
  }

  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Model info */
  .model-info {
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 8px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .model-info-row {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
  }

  .info-label {
    color: var(--text-secondary);
    font-weight: 500;
  }

  .info-value {
    color: var(--text-primary);
  }

  /* Footer */
  .settings-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid var(--border-light);
  }

  .settings-card.embedded .settings-footer {
    padding: 18px 26px 22px;
  }

  .btn {
    padding: 8px 20px;
    font-size: 13px;
    font-weight: 500;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, box-shadow 0.15s;
  }

  .btn-secondary {
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
  }

  .btn-secondary:hover {
    background: var(--bg-tertiary);
    color: var(--text-primary);
  }

  .btn-primary {
    color: var(--on-accent);
    background: var(--accent);
    border: 1px solid var(--accent);
  }

  .btn-primary:hover {
    background: var(--accent-hover);
    box-shadow: 0 2px 8px var(--accent-glow);
  }

  /* Spinning animation */
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .spinning {
    animation: spin 0.8s linear infinite;
  }

  /* Model download status */
  .model-status-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 12px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .badge-success {
    background: color-mix(in srgb, var(--success) 15%, transparent);
    color: var(--success);
    border: 1px solid color-mix(in srgb, var(--success) 30%, transparent);
  }

  .badge-warning {
    background: color-mix(in srgb, var(--warning) 15%, transparent);
    color: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 30%, transparent);
  }

  .btn-sm {
    padding: 4px 12px;
    font-size: 12px;
  }

  .btn-danger-outline {
    color: var(--danger);
    border-color: var(--danger);
  }

  .btn-danger-outline:hover {
    background: var(--danger);
    color: var(--on-danger);
  }

  .download-progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 4px;
  }

  .progress-bar {
    width: 100%;
    height: 4px;
    background: var(--bg-tertiary);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-indeterminate {
    width: 40%;
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    animation: indeterminate 1.4s ease-in-out infinite;
  }

  @keyframes indeterminate {
    0% { transform: translateX(-100%); }
    50% { transform: translateX(150%); }
    100% { transform: translateX(400%); }
  }

  .field-error {
    font-size: 11.5px;
    color: var(--danger);
    margin: 0;
  }

  /* Mic test */
  .mic-test-container {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 10px;
  }

  .mic-test-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    border-radius: 7px;
    cursor: pointer;
    white-space: nowrap;
    color: var(--accent);
    background: transparent;
    border: 1px solid var(--accent);
    transition: background 0.15s, color 0.15s, border-color 0.15s, box-shadow 0.15s;
  }

  .mic-test-toggle:hover {
    background: var(--accent);
    color: var(--on-accent);
  }

  .mic-test-toggle.active {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--on-danger);
  }

  .mic-test-toggle.active:hover {
    background: var(--danger-hover);
    border-color: var(--danger-hover);
  }

  .mic-pill-track {
    flex: 1;
    height: 22px;
    min-width: 0;
    border-radius: 99px;
    background: var(--bg-tertiary);
    overflow: hidden;
    border: 1px solid var(--border-light);
  }

  .mic-pill-fill {
    height: 100%;
    border-radius: 99px;
    background: var(--success);
    transition: width 0.04s linear, background 0.15s ease, box-shadow 0.15s ease;
    box-shadow: 0 0 0 transparent;
  }

  .mic-pill-fill.loud {
    background: var(--warning);
    box-shadow: 0 0 14px color-mix(in srgb, var(--warning) 45%, transparent), inset 0 0 6px var(--highlight);
  }

  .mic-hint {
    animation: mic-hint-in 0.25s ease;
  }

  @keyframes mic-hint-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (max-width: 840px) {
    .settings-card.embedded {
      border-radius: 18px;
    }

    .settings-card.embedded .settings-header,
    .settings-card.embedded .tab-panel,
    .settings-card.embedded .tabs,
    .settings-card.embedded .settings-footer {
      padding-left: 18px;
      padding-right: 18px;
    }

    .settings-card.embedded .settings-header {
      padding-top: 18px;
      padding-bottom: 14px;
    }

    .settings-card.embedded .settings-header h2 {
      font-size: 24px;
    }
  }

  /* Range / slider input */
  .field-range {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-tertiary, #d1d5db);
    outline: none;
    transition: background 0.15s;
  }
  .field-range::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--accent, #2563eb);
    cursor: pointer;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
    transition: transform 0.1s, box-shadow 0.15s;
  }
  .field-range::-webkit-slider-thumb:hover {
    transform: scale(1.15);
    box-shadow: 0 2px 8px var(--accent-glow, rgba(37, 99, 235, 0.35));
  }
  .field-range::-moz-range-thumb {
    width: 16px;
    height: 16px;
    border: none;
    border-radius: 50%;
    background: var(--accent, #2563eb);
    cursor: pointer;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
  }
  .field-range::-moz-range-track {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-tertiary, #d1d5db);
  }

  /* Tab dot (update indicator) */
  .tab-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    margin-left: 4px;
    vertical-align: middle;
    box-shadow: 0 0 4px var(--accent-glow, rgba(37, 99, 235, 0.4));
  }

  /* About tab */
  .about-panel {
    gap: 18px;
  }

  .about-header {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .about-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
    line-height: 1.2;
  }

  .about-version {
    font-size: 12px;
    color: var(--text-muted);
    margin: 2px 0 0;
  }

  .about-description {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.55;
    margin: 0;
  }

  .about-divider {
    height: 1px;
    background: var(--border-light);
  }

  .about-link-row {
    display: flex;
    gap: 10px;
  }

  .about-link {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 500;
    color: var(--accent);
    text-decoration: none;
    padding: 6px 12px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 8%, transparent);
    transition: background 0.15s, color 0.15s;
  }

  .about-link:hover {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  .about-up-to-date {
    font-size: 13px;
    color: var(--text-muted);
    margin: 0;
    padding: 8px 0;
  }

  .update-no-update-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .about-info-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .about-info-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 14px;
    border-radius: 8px;
    background: var(--bg-secondary);
  }

  .about-info-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    color: var(--text-muted);
  }

  .about-info-value {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary);
  }

  /* Update card */
  .update-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 6%, var(--bg-secondary));
    border: 1px solid color-mix(in srgb, var(--accent) 20%, var(--border-light));
  }

  .update-card-header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .update-badge {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.3px;
    color: var(--on-accent, #fff);
    background: var(--accent);
    padding: 3px 8px;
    border-radius: 5px;
  }

  .update-version {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .update-notes {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 0;
    line-height: 1.5;
    white-space: pre-wrap;
    max-height: 120px;
    overflow-y: auto;
  }

  .update-btn {
    align-self: flex-start;
  }

  .update-progress-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .update-progress-bar {
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-tertiary);
    overflow: hidden;
  }

  .update-progress-fill {
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 0.15s ease;
  }

  .update-progress-text {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary);
    min-width: 32px;
    text-align: right;
  }
</style>
