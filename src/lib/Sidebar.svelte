<script>
  import QuillIcon from './QuillIcon.svelte';

  let {
    activePanel = 'record',
    sessionCount = 0,
    hasUpdate = false,
    onnavigate,
  } = $props();
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <div class="sidebar-logo">
      <div class="sidebar-logo-icon">
        <QuillIcon width={20} height={20} />
      </div>
      <div>
        <h1><span class="logo-accent">Quill</span>Scribe</h1>
        <p class="sidebar-tagline">Voice Workspace</p>
      </div>
    </div>
  </div>

  <nav class="sidebar-nav">
    <p class="sidebar-section-label">Navigation</p>
    <button
      class="nav-button"
      class:active={activePanel === 'record'}
      onclick={() => onnavigate('record')}
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

    <button class="nav-button" class:active={activePanel === 'settings'} onclick={() => onnavigate('settings')}>
      <span class="nav-icon">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      </span>
      <span class="nav-label">Settings</span>
      {#if hasUpdate}
        <span class="update-dot" title="Update available"></span>
      {/if}
    </button>

    <button
      class="nav-button"
      class:active={activePanel === 'history'}
      onclick={() => onnavigate('history')}
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
      <span class="user-meta">{sessionCount} sessions</span>
    </div>
  </div>
</aside>

<style>
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

  .sidebar-logo {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .sidebar-logo-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    flex-shrink: 0;
  }

  .logo-accent {
    color: var(--accent);
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

  .sidebar-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-button {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease;
    width: 100%;
    padding: 9px 10px;
    justify-content: flex-start;
  }

  .nav-button:hover {
    background: color-mix(in srgb, var(--bg-primary) 80%, var(--bg-secondary));
    color: var(--text-primary);
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

  .update-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
    margin-left: auto;
    box-shadow: 0 0 6px var(--accent-glow, rgba(37, 99, 235, 0.4));
    animation: dot-pulse 2s ease-in-out infinite;
  }

  @keyframes dot-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
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

  @media (max-width: 840px) {
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
  }
</style>
