<script>
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { isDark = false } = $props();

  async function minimize() {
    await getCurrentWindow().minimize();
  }

  async function close() {
    await getCurrentWindow().close();
  }
</script>

<div class="titlebar" data-tauri-drag-region>
  <div class="titlebar-left" data-tauri-drag-region>
    <div class="titlebar-icon" data-tauri-drag-region>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 2L2 7l10 5 10-5-10-5z" />
        <path d="M2 17l10 5 10-5" />
        <path d="M2 12l10 5 10-5" />
      </svg>
    </div>
    <span class="titlebar-title" data-tauri-drag-region>QuillScribe</span>
  </div>

  <div class="titlebar-controls">
    <button class="titlebar-btn minimize-btn" onclick={minimize} aria-label="Minimize">
      <svg width="10" height="1" viewBox="0 0 10 1">
        <rect width="10" height="1" fill="currentColor" />
      </svg>
    </button>
    <button class="titlebar-btn close-btn" onclick={close} aria-label="Close">
      <svg width="10" height="10" viewBox="0 0 10 10">
        <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1.2" />
        <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1.2" />
      </svg>
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--titlebar-bg);
    color: var(--titlebar-text);
    padding-left: 10px;
    flex-shrink: 0;
    -webkit-user-select: none;
    user-select: none;
  }

  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    height: 100%;
  }

  .titlebar-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
  }

  .titlebar-title {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.3px;
  }

  .titlebar-controls {
    display: flex;
    height: 100%;
  }

  .titlebar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--titlebar-text);
    cursor: pointer;
    padding: 0;
    transition: background-color 0.15s ease;
  }

  .titlebar-btn:hover {
    background: rgba(128, 128, 128, 0.2);
  }

  .close-btn:hover {
    background: #e81123;
    color: #ffffff;
  }
</style>
