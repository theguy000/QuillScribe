<script>
  import { tick } from 'svelte'

  let {
    value = '',
    options = [],
    onchange = () => {},
    class: className = '',
  } = $props()

  let open = $state(false)
  let containerEl = $state(null)
  let listEl = $state(null)
  let highlightIndex = $state(-1)

  const selectedLabel = $derived(
    options.find(o => String(o.value) === String(value))?.label ?? value
  )

  function toggle() {
    open = !open
    highlightIndex = -1
  }

  function select(opt) {
    open = false
    highlightIndex = -1
    // Mimic native <select> onchange event shape
    onchange({ currentTarget: { value: opt.value } })
  }

  function handleClickOutside(e) {
    if (containerEl && !containerEl.contains(e.target) && e.target !== listEl && !listEl?.contains(e.target)) {
      open = false
      highlightIndex = -1
    }
  }

  function handleTriggerKeydown(e) {
    if (!open && (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault()
      open = true
      highlightIndex = options.findIndex(o => String(o.value) === String(value))
      tick().then(() => scrollToHighlighted())
    } else if (open) {
      handleListKeydown(e)
    }
  }

  function handleListKeydown(e) {
    if (!open) return

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault()
        highlightIndex = highlightIndex < options.length - 1 ? highlightIndex + 1 : 0
        scrollToHighlighted()
        break
      case 'ArrowUp':
        e.preventDefault()
        highlightIndex = highlightIndex > 0 ? highlightIndex - 1 : options.length - 1
        scrollToHighlighted()
        break
      case 'Home':
        e.preventDefault()
        highlightIndex = 0
        scrollToHighlighted()
        break
      case 'End':
        e.preventDefault()
        highlightIndex = options.length - 1
        scrollToHighlighted()
        break
      case 'Enter':
      case ' ':
        e.preventDefault()
        if (highlightIndex >= 0 && highlightIndex < options.length) {
          select(options[highlightIndex])
        }
        break
      case 'Escape':
      case 'Tab':
        e.preventDefault()
        close()
        break
    }
  }

  function scrollToHighlighted() {
    if (!listEl || highlightIndex < 0) return
    const item = listEl.children[highlightIndex]
    item?.scrollIntoView({ block: 'nearest' })
  }

  function updateDropdownPosition() {
    if (!containerEl || !listEl) return
    const rect = containerEl.getBoundingClientRect()
    listEl.style.position = 'fixed'
    listEl.style.top = `${rect.bottom}px`
    listEl.style.left = `${rect.left}px`
    listEl.style.width = `${rect.width}px`
  }

  function handleScroll() {
    if (!open) return
    if (!containerEl) { close(); return }
    const rect = containerEl.getBoundingClientRect()
    // Close if trigger scrolled out of view
    if (rect.bottom < 0 || rect.top > window.innerHeight || rect.width === 0) {
      close()
      return
    }
    updateDropdownPosition()
  }

  function close() {
    open = false
    highlightIndex = -1
  }

  $effect(() => {
    if (open) {
      window.addEventListener('mousedown', handleClickOutside)
      window.addEventListener('scroll', handleScroll, true)
      tick().then(() => updateDropdownPosition())
    } else {
      window.removeEventListener('mousedown', handleClickOutside)
      window.removeEventListener('scroll', handleScroll, true)
    }
    return () => {
      window.removeEventListener('mousedown', handleClickOutside)
      window.removeEventListener('scroll', handleScroll, true)
    }
  })
</script>

<svelte:window onresize={() => open && updateDropdownPosition()} />

<div class="custom-select" class:open bind:this={containerEl}>
  <button
    type="button"
    class="custom-select-trigger {className}"
    onclick={toggle}
    onkeydown={handleTriggerKeydown}
    aria-haspopup="listbox"
    aria-expanded={open}
  >
    <span class="custom-select-value">{selectedLabel}</span>
    <svg class="chevron" width="12" height="12" viewBox="0 0 12 12" fill="none">
      <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>
  </button>
</div>

{#if open}
  <ul class="custom-select-options" bind:this={listEl} role="listbox" onkeydown={handleListKeydown}>
    {#each options as opt, i}
      <li
        role="option"
        class="custom-select-option"
        class:selected={String(opt.value) === String(value)}
        class:highlighted={i === highlightIndex}
        aria-selected={String(opt.value) === String(value)}
        onclick={() => select(opt)}
        onmouseenter={() => highlightIndex = i}
        tabindex="-1"
      >
        {opt.label}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .custom-select {
    position: relative;
    width: 100%;
  }

  .custom-select-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    max-width: 340px;
    padding: 8px 12px;
    font-size: 13px;
    font-family: inherit;
    color: var(--text-primary);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    outline: none;
    cursor: pointer;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .custom-select-trigger:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }

  .custom-select.open .custom-select-trigger {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-glow);
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }

  .custom-select-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chevron {
    flex-shrink: 0;
    margin-left: 8px;
    transition: transform 0.15s;
  }

  .custom-select.open .chevron {
    transform: rotate(180deg);
  }

  .custom-select-options {
    position: fixed;
    z-index: 10000;
    max-width: 340px;
    max-height: 220px;
    overflow-y: auto;
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--bg-secondary);
    border: 1px solid var(--accent);
    border-top: none;
    border-bottom-left-radius: 8px;
    border-bottom-right-radius: 8px;
    box-shadow: 0 4px 12px var(--shadow-lg);
    box-sizing: border-box;
  }

  .custom-select-option {
    padding: 7px 12px;
    font-size: 13px;
    color: var(--text-primary);
    cursor: pointer;
    transition: background 0.1s;
  }

  .custom-select-option:hover,
  .custom-select-option.highlighted {
    background: var(--bg-tertiary);
  }

  .custom-select-option.selected {
    font-weight: 600;
    color: var(--accent);
  }
</style>
