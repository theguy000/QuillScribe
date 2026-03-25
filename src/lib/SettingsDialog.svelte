<script>
  import { invoke } from '@tauri-apps/api/core'

  let { show, onclose, settings, onsave } = $props()

  let activeTab = $state('audio')
  let localSettings = $state(null)
  let audioDevices = $state([])
  let languages = $state([])
  let localModels = $state([])
  let modelInfo = $state(null)
  let showApiKey = $state(false)
  let modelCategory = $state('All')
  let testingMic = $state(false)
  let loadingDevices = $state(false)

  const tabs = [
    { id: 'audio', label: 'Audio' },
    { id: 'whisper', label: 'Whisper' },
    { id: 'ui', label: 'UI' },
    { id: 'output', label: 'Output' },
    { id: 'keyboard', label: 'Keyboard' },
    { id: 'statistics', label: 'Statistics' },
  ]

  const themes = [
    { value: 'white', label: 'White' },
    { value: 'warm_gray', label: 'Warm Gray' },
    { value: 'soft_beige', label: 'Soft Beige' },
    { value: 'blue_gray', label: 'Blue Gray' },
    { value: 'warm_taupe', label: 'Warm Taupe' },
    { value: 'soft_sage', label: 'Soft Sage' },
    { value: 'dark_charcoal', label: 'Dark Charcoal' },
    { value: 'dark_blue', label: 'Dark Blue' },
    { value: 'dark_purple', label: 'Dark Purple' },
    { value: 'dark_forest', label: 'Dark Forest' },
    { value: 'dark_burgundy', label: 'Dark Burgundy' },
    { value: 'obsidian', label: 'Obsidian' },
  ]

  const outputModes = [
    { value: 0, label: 'Copy Only' },
    { value: 1, label: 'Paste Only' },
    { value: 2, label: 'Copy & Paste' },
    { value: 3, label: 'Display Only' },
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

  $effect(() => {
    if (show && settings) {
      localSettings = JSON.parse(JSON.stringify(settings))
      loadInitialData()
    }
  })

  async function loadInitialData() {
    await Promise.all([
      loadAudioDevices(),
      loadLanguages(),
      loadLocalModels(),
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
      languages = await invoke('get_available_languages')
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
      modelInfo = await invoke('get_model_info', { model: modelName })
    } catch (e) {
      console.error('Failed to load model info:', e)
      modelInfo = null
    }
  }

  async function testMicrophone() {
    testingMic = true
    try {
      await invoke('test_microphone')
    } catch (e) {
      console.error('Microphone test failed:', e)
    } finally {
      testingMic = false
    }
  }

  function handleSave() {
    onsave(localSettings)
  }

  function handleCancel() {
    onclose()
  }

  function handleOverlayClick(e) {
    if (e.target === e.currentTarget) {
      onclose()
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Escape') {
      onclose()
    }
  }

  function handleLocalModelChange(value) {
    localSettings.whisper.local_model = value
    loadModelInfo(value)
  }
</script>

{#if show && localSettings}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" onclick={handleOverlayClick} onkeydown={handleKeydown}>
    <div class="settings-card">
      <div class="settings-header">
        <h2>Settings</h2>
        <button class="close-btn" onclick={onclose} aria-label="Close">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M1 1L13 13M1 13L13 1" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
          </svg>
        </button>
      </div>

      <nav class="tabs">
        {#each tabs as tab}
          <button
            class="tab"
            class:active={activeTab === tab.id}
            onclick={() => activeTab = tab.id}
          >
            {tab.label}
          </button>
        {/each}
      </nav>

      <div class="tab-content">
        <!-- Audio Tab -->
        {#if activeTab === 'audio'}
          <div class="tab-panel">
            <div class="field">
              <label class="field-label">Microphone</label>
              <div class="field-row">
                <select
                  class="field-select"
                  value={localSettings.audio.device_id}
                  onchange={(e) => localSettings.audio.device_id = e.target.value ? Number(e.target.value) : null}
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
              <button class="action-btn" onclick={testMicrophone} disabled={testingMic}>
                {testingMic ? 'Testing...' : 'Test Microphone'}
              </button>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.audio.auto_select_mic}
                  onchange={(e) => localSettings.audio.auto_select_mic = e.target.checked}
                />
                <span>Auto-select microphone</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.audio.sounds_enabled}
                  onchange={(e) => localSettings.audio.sounds_enabled = e.target.checked}
                />
                <span>Enable sounds</span>
              </label>
            </div>

            <div class="field">
              <label class="field-label">Microphone Blocklist</label>
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
              <label class="field-label">Mode</label>
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
                <label class="field-label">API Key</label>
                <div class="field-row">
                  <input
                    class="field-input"
                    type={showApiKey ? 'text' : 'password'}
                    value={localSettings.whisper.api_key}
                    oninput={(e) => localSettings.whisper.api_key = e.target.value}
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
                <label class="field-label">API Model</label>
                <select
                  class="field-select"
                  value={localSettings.whisper.api_model}
                  onchange={(e) => localSettings.whisper.api_model = e.target.value}
                >
                  {#each apiModels as model}
                    <option value={model}>{model}</option>
                  {/each}
                </select>
              </div>

              <div class="field">
                <label class="field-label">Language</label>
                <select
                  class="field-select"
                  value={localSettings.whisper.api_language}
                  onchange={(e) => localSettings.whisper.api_language = e.target.value}
                >
                  {#each languages as lang}
                    <option value={lang.code || lang}>{lang.name || lang}</option>
                  {/each}
                </select>
              </div>
            {:else}
              <div class="field">
                <label class="field-label">Category</label>
                <select
                  class="field-select"
                  value={modelCategory}
                  onchange={(e) => modelCategory = e.target.value}
                >
                  {#each modelCategories as cat}
                    <option value={cat}>{cat}</option>
                  {/each}
                </select>
              </div>

              <div class="field">
                <label class="field-label">Model</label>
                <select
                  class="field-select"
                  value={localSettings.whisper.local_model}
                  onchange={(e) => handleLocalModelChange(e.target.value)}
                >
                  {#each filteredModels as model}
                    <option value={model.id || model}>{model.name || model}</option>
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
              <label class="field-label">Theme</label>
              <select
                class="field-select"
                value={localSettings.ui.theme}
                onchange={(e) => localSettings.ui.theme = e.target.value}
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
                  checked={localSettings.ui.show_waveform}
                  onchange={(e) => localSettings.ui.show_waveform = e.target.checked}
                />
                <span>Show waveform</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.compact_mode}
                  onchange={(e) => localSettings.ui.compact_mode = e.target.checked}
                />
                <span>Compact mode</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.custom_titlebar}
                  onchange={(e) => localSettings.ui.custom_titlebar = e.target.checked}
                />
                <span>Custom titlebar</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.always_on_top}
                  onchange={(e) => localSettings.ui.always_on_top = e.target.checked}
                />
                <span>Always on top</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.minimize_on_close}
                  onchange={(e) => localSettings.ui.minimize_on_close = e.target.checked}
                />
                <span>Minimize on close</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.ui.minimize_to_tray}
                  onchange={(e) => localSettings.ui.minimize_to_tray = e.target.checked}
                />
                <span>Minimize to tray</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input type="checkbox" checked={localSettings.ui.snap_to_edges ?? true}
                  onchange={(e) => localSettings.ui.snap_to_edges = e.target.checked} />
                <span>Snap to edges</span>
              </label>
            </div>

            <div class="field">
              <label class="field-label">Animation Strength</label>
              <div class="slider-row">
                <input type="range" class="field-range" min="1" max="10"
                  value={localSettings.ui.animation_strength ?? 3}
                  oninput={(e) => localSettings.ui.animation_strength = Number(e.target.value)} />
                <span class="slider-value">{localSettings.ui.animation_strength ?? 3}x</span>
              </div>
              <p class="field-hint">Higher values make the waveform more responsive to quiet voices.</p>
            </div>
          </div>
        {/if}

        <!-- Output Tab -->
        {#if activeTab === 'output'}
          <div class="tab-panel">
            <div class="field">
              <label class="field-label">Output Mode</label>
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
                    <span>{mode.label}</span>
                  </label>
                {/each}
              </div>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.output.silent_mode}
                  onchange={(e) => localSettings.output.silent_mode = e.target.checked}
                />
                <span>Silent mode</span>
              </label>
            </div>

            <div class="field">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  checked={localSettings.output.auto_clear}
                  onchange={(e) => localSettings.output.auto_clear = e.target.checked}
                />
                <span>Auto-clear clipboard</span>
              </label>
            </div>

            {#if localSettings.output.auto_clear}
              <div class="field">
                <label class="field-label">Auto-clear delay (seconds)</label>
                <input
                  class="field-input field-input-short"
                  type="number"
                  min="1"
                  max="300"
                  value={localSettings.output.auto_clear_delay}
                  oninput={(e) => localSettings.output.auto_clear_delay = Number(e.target.value)}
                />
              </div>
            {/if}
          </div>
        {/if}

        <!-- Keyboard Tab -->
        {#if activeTab === 'keyboard'}
          <div class="tab-panel">
            <div class="field">
              <label class="field-label">Record Toggle Shortcut</label>
              <input
                class="field-input"
                type="text"
                value={localSettings.shortcuts.record_toggle}
                oninput={(e) => localSettings.shortcuts.record_toggle = e.target.value}
                placeholder="e.g. Meta+Shift+`"
              />
              <p class="field-hint">Current shortcut: <code>{localSettings.shortcuts.record_toggle}</code></p>
            </div>
          </div>
        {/if}

        <!-- Statistics Tab -->
        {#if activeTab === 'statistics'}
          <div class="tab-panel">
            <div class="stats-section">
              <h3 class="section-title">Usage Statistics</h3>
              <div class="stats-grid">
                <div class="stat-card">
                  <span class="stat-label">Total Sessions</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Total Recordings</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Total Duration</span>
                  <span class="stat-value">0h 0m</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Success Rate</span>
                  <span class="stat-value">0.0%</span>
                </div>
              </div>
            </div>

            <div class="stats-section">
              <h3 class="section-title">Performance Metrics</h3>
              <div class="stats-grid">
                <div class="stat-card">
                  <span class="stat-label">Avg Transcription Time</span>
                  <span class="stat-value">0.00s</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Fastest</span>
                  <span class="stat-value">N/A</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Slowest</span>
                  <span class="stat-value">0.00s</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Avg Audio Duration</span>
                  <span class="stat-value">0.00s</span>
                </div>
              </div>
            </div>

            <div class="stats-section">
              <h3 class="section-title">Mode Usage</h3>
              <div class="stats-grid three-col">
                <div class="stat-card">
                  <span class="stat-label">API Mode</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Local Mode</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Total Characters</span>
                  <span class="stat-value">0</span>
                </div>
              </div>
            </div>

            <div class="stats-section">
              <h3 class="section-title">Current Session</h3>
              <div class="stats-grid">
                <div class="stat-card">
                  <span class="stat-label">Duration</span>
                  <span class="stat-value">0h 0m</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Recordings</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Successful</span>
                  <span class="stat-value">0</span>
                </div>
                <div class="stat-card">
                  <span class="stat-label">Failed</span>
                  <span class="stat-value">0</span>
                </div>
              </div>
            </div>

            <div class="stats-section">
              <h3 class="section-title">Recent History</h3>
              <textarea class="history-area" readonly>No recent transcriptions</textarea>
            </div>

            <div class="stats-actions">
              <button class="action-btn" onclick={() => {}}>Refresh</button>
              <button class="action-btn" onclick={() => {}}>Export Statistics</button>
              <button class="action-btn btn-danger" onclick={() => {}}>Reset Statistics</button>
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

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px 12px;
    border-bottom: 1px solid var(--border-light);
  }

  .settings-header h2 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
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

  .tab-panel {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
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

  .field-input-short {
    max-width: 100px;
  }

  .field-hint {
    font-size: 11.5px;
    color: var(--text-muted);
    margin: 0;
  }

  .field-hint code {
    background: var(--bg-tertiary);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 11px;
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
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
    flex-shrink: 0;
  }

  /* Radio */
  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .radio-label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
    color: var(--text-primary);
  }

  .radio-label input[type="radio"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
    cursor: pointer;
    flex-shrink: 0;
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
    color: #fff;
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
    color: #fff;
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

  /* Statistics tab */
  .stats-section {
    margin-bottom: 20px;
  }

  .section-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.4px;
    margin: 0 0 10px 0;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .stats-grid.three-col {
    grid-template-columns: 1fr 1fr 1fr;
  }

  .stat-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 10px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .stat-value {
    font-size: 22px;
    font-weight: 700;
    color: var(--text-primary);
    font-family: 'Segoe UI', system-ui, sans-serif;
  }

  .history-area {
    width: 100%;
    min-height: 100px;
    max-height: 140px;
    padding: 10px 12px;
    font-size: 11.5px;
    font-family: 'Consolas', 'Monaco', monospace;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    resize: none;
    outline: none;
  }

  .stats-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .btn-danger {
    color: #fff;
    background: #dc3545;
    border: 1px solid #dc3545;
  }

  .btn-danger:hover {
    background: #c82333;
  }

  /* Slider row */
  .slider-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .field-range {
    flex: 1;
    height: 6px;
    accent-color: var(--accent);
    cursor: pointer;
  }

  .slider-value {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    min-width: 30px;
    text-align: center;
  }
</style>
