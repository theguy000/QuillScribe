<script>
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import QuillIcon from './QuillIcon.svelte';

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
      <QuillIcon width={16} height={16} />
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
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: linear-gradient(180deg, color-mix(in srgb, var(--titlebar-bg) 96%, var(--bg-primary) 4%), var(--titlebar-bg));
    color: var(--titlebar-text);
    padding-left: 14px;
    border-bottom: 1px solid var(--border-light);
    box-shadow: inset 0 -1px 0 color-mix(in srgb, var(--border-light) 75%, transparent);
    flex-shrink: 0;
    -webkit-user-select: none;
    user-select: none;
  }

  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    height: 100%;
  }

  .titlebar-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
    width: 24px;
    height: 24px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  .titlebar-title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .titlebar-controls {
    display: flex;
    height: 100%;
  }

  .titlebar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--titlebar-text);
    cursor: pointer;
    padding: 0;
    transition: background-color 0.15s ease;
  }

  .titlebar-btn:hover {
    background: color-mix(in srgb, var(--bg-tertiary) 78%, transparent);
  }

  .close-btn:hover {
    background: var(--danger);
    color: var(--on-danger);
  }
</style>
