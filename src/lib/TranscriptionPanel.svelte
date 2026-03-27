<script>
  let {
    transcriptionText = $bindable(''),
  } = $props();

  let isEditing = $state(false);

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
</script>

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

<style>
  .transcription-section {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
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

  .empty-inline {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    flex: 1;
    min-height: 0;
  }

  .empty-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .empty-copy {
    color: var(--text-secondary);
  }
</style>
