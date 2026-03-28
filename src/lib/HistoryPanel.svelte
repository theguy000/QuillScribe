<script>
  let {
    historyEntries = [],
    historyLoading = false,
    historyError = '',
    onrefresh,
  } = $props();

  let expandedHistoryIndex = $state(-1);
  let compactView = $state(false);

  let recentHistory = $derived([...historyEntries].reverse());

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
    if (seconds == null || Number.isNaN(seconds)) return '\u2014';
    if (seconds < 60) {
      return `${seconds.toFixed(1)}s`;
    }
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = Math.round(seconds % 60)
      .toString()
      .padStart(2, '0');
    return `${minutes}:${remainingSeconds}`;
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

<div class="panel history-panel">
  <div class="panel-header">
    <div>
      <p class="panel-kicker">Archive</p>
      <h2>Recent history</h2>
    </div>
    <div class="panel-actions">
      <button
        class="panel-action icon-btn"
        class:active={compactView}
        onclick={() => compactView = !compactView}
        title={compactView ? 'Switch to detailed view' : 'Switch to compact view'}
      >
        {#if compactView}
          <!-- detailed/expand icon -->
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" /><rect x="3" y="14" width="7" height="7" /><rect x="14" y="14" width="7" height="7" />
          </svg>
        {:else}
          <!-- compact/list icon -->
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="8" y1="6" x2="21" y2="6" /><line x1="8" y1="12" x2="21" y2="12" /><line x1="8" y1="18" x2="21" y2="18" /><line x1="3" y1="6" x2="3.01" y2="6" /><line x1="3" y1="12" x2="3.01" y2="12" /><line x1="3" y1="18" x2="3.01" y2="18" />
          </svg>
        {/if}
      </button>
      <button class="panel-action" onclick={onrefresh} disabled={historyLoading}>
        {historyLoading ? 'Refreshing...' : 'Refresh'}
      </button>
    </div>
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
        {#if compactView}
          <div class="history-item compact">
            <div class="compact-row">
              <p class="compact-text">{entry.text || 'No transcription text'}</p>
              <span class="compact-time">{formatTimestamp(entry.timestamp)}</span>
              <button
                class="action-btn compact-copy"
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
          </div>
        {:else}
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
              <!-- confidence omitted: currently hardcoded to 100% -->
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
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
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

  .history-panel {
    min-height: 0;
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

  .panel-kicker {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: var(--text-muted);
  }

  .panel-actions {
    display: flex;
    gap: 8px;
  }

  .panel-action {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-primary) 92%, transparent);
    color: var(--text-primary);
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease, border-color 0.12s ease;
    padding: 10px 14px;
    font-size: 13px;
    font-weight: 600;
  }

  .panel-action:hover {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border));
    background: color-mix(in srgb, var(--bg-primary) 98%, var(--accent) 2%);
  }

  .panel-action.icon-btn {
    padding: 8px;
    line-height: 0;
  }

  .panel-action.icon-btn svg {
    display: block;
  }

  .panel-action.active {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-primary));
    color: var(--accent);
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    padding-right: 8px;
  }

  .history-item {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 12px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
    padding: 12px 14px;
    box-shadow: 0 4px 12px color-mix(in srgb, var(--shadow) 22%, transparent);
    cursor: pointer;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
  }

  .history-item:hover {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border-light));
  }

  .history-item.compact {
    padding: 10px 14px;
    cursor: default;
  }

  .compact-row {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .compact-text {
    flex: 1;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-primary);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    word-wrap: break-word;
  }

  .compact-copy {
    flex-shrink: 0;
  }

  .compact-time {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-muted);
    white-space: nowrap;
    padding-top: 1px;
  }

  .history-item.expanded {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--border-light));
    box-shadow: 0 4px 12px color-mix(in srgb, var(--shadow) 22%, transparent),
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
    gap: 12px;
    margin-bottom: 6px;
  }

  .history-time {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 2px;
  }

  .history-mode {
    font-size: 12px;
    text-transform: capitalize;
    color: var(--text-secondary);
  }

  .history-badge {
    padding: 4px 8px;
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
    gap: 8px;
    font-size: 12px;
    color: var(--text-secondary);
  }

  .history-metrics span {
    padding: 4px 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--bg-primary) 82%, transparent);
    border: 1px solid color-mix(in srgb, var(--border-light) 85%, transparent);
  }

  .history-detail {
    margin-top: 10px;
    padding-top: 10px;
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

  .empty-state {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    min-height: 220px;
    padding: 28px;
    background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, transparent), color-mix(in srgb, var(--bg-secondary) 86%, transparent));
  }

  .empty-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .empty-copy {
    color: var(--text-secondary);
  }

  @media (max-width: 840px) {
    .panel-header,
    .history-item-top {
      flex-direction: column;
    }

    .panel {
      height: auto;
      min-height: 100%;
    }
  }
</style>
