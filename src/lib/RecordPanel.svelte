<script>
  let {
    isRecording = false,
    isTranscribing = false,
    statusMessage = 'Ready',
    audioLevel = 0,
    showBurst = false,
    ontogglerecording,
  } = $props();
</script>

<section class="record-card">
  <button
    class="mic-button"
    class:recording={isRecording}
    class:transcribing={isTranscribing}
    onclick={ontogglerecording}
    disabled={isTranscribing}
    aria-label={isTranscribing ? 'Transcribing...' : isRecording ? 'Stop recording' : 'Start recording'}
  >
    {#if isRecording}
      <div class="wave-ring wave-ring-1"></div>
      <div class="wave-ring wave-ring-2"></div>
      <div class="wave-ring wave-ring-3"></div>
    {/if}
    {#if isTranscribing}
      <svg class="dual-spinner" width="108" height="108" viewBox="0 0 108 108">
        <circle class="spinner-arc-slow" cx="54" cy="54" r="50" />
        <circle class="spinner-arc-fast" cx="54" cy="54" r="50" />
      </svg>
    {/if}
    {#if showBurst}
      <div class="burst-ring burst-ring-1"></div>
      <div class="burst-ring burst-ring-2"></div>
      <div class="burst-ring burst-ring-3"></div>
      <div class="burst-flash"></div>
    {/if}
    <div
      class="mic-ring"
      class:recording={isRecording}
      class:transcribing={isTranscribing}
      style:transform="scale({isRecording ? 1 + audioLevel * 0.08 : 1})"
    >
      {#if isRecording}
        <div class="mic-icon-wrap recording">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="none">
            <rect x="6" y="6" width="12" height="12" rx="3" />
          </svg>
        </div>
      {:else}
        <div class="mic-icon-wrap">
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
      {/if}
    </div>
  </button>

  <div class="record-info">
    <p class="record-title">
      {#if isTranscribing}
        <span class="transcribing-dots">Transcribing<span class="dot">.</span><span class="dot">.</span><span class="dot">.</span></span>
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

<style>
  .record-card {
    border: 1px solid color-mix(in srgb, var(--border-light) 95%, transparent);
    border-radius: 18px;
    background: color-mix(in srgb, var(--bg-secondary) 80%, var(--bg-primary));
    display: grid;
    grid-template-rows: 1fr auto 1fr;
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

  .record-card::before {
    content: '';
    display: block;
  }

  .record-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    align-self: start;
    gap: 4px;
    padding-top: 16px;
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

  .wave-ring {
    position: absolute;
    width: 88px;
    height: 88px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    pointer-events: none;
    opacity: 0;
    animation: wave-ripple 2.4s cubic-bezier(0.2, 0.6, 0.35, 1) infinite;
  }

  .wave-ring-1 { animation-delay: 0s; }
  .wave-ring-2 { animation-delay: 0.8s; }
  .wave-ring-3 { animation-delay: 1.6s; }

  @keyframes wave-ripple {
    0% { transform: scale(1); opacity: 0.55; }
    100% { transform: scale(2.2); opacity: 0; }
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
    box-shadow: 0 0 20px color-mix(in srgb, var(--accent) 35%, transparent);
  }

  .mic-button.transcribing {
    cursor: wait;
    pointer-events: none;
  }

  .mic-button:disabled {
    cursor: not-allowed;
  }

  .mic-button:active:not(:disabled) .mic-ring {
    transform: scale(0.9) !important;
    transition: transform 0.1s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .mic-icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    animation: icon-fade-in 0.25s ease both;
  }

  @keyframes icon-fade-in {
    from { opacity: 0; transform: scale(0.6); }
    to { opacity: 1; transform: scale(1); }
  }

  .dual-spinner {
    position: absolute;
    z-index: 2;
    pointer-events: none;
  }

  .spinner-arc-slow {
    fill: none;
    stroke: var(--accent);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-dasharray: 100 214;
    transform-origin: center;
    animation: spin-slow 2s linear infinite;
  }

  .spinner-arc-fast {
    fill: none;
    stroke: color-mix(in srgb, var(--accent) 35%, transparent);
    stroke-width: 2.5;
    stroke-linecap: round;
    stroke-dasharray: 40 274;
    transform-origin: center;
    animation: spin-fast 1.2s linear infinite;
  }

  @keyframes spin-slow {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @keyframes spin-fast {
    from { transform: rotate(0deg); }
    to { transform: rotate(-360deg); }
  }

  .mic-ring.transcribing {
    opacity: 0.5;
  }

  .transcribing-dots .dot {
    display: inline-block;
    opacity: 0;
    animation: dot-pulse 1.4s ease-in-out infinite;
  }

  .transcribing-dots .dot:nth-child(1) { animation-delay: 0s; }
  .transcribing-dots .dot:nth-child(2) { animation-delay: 0.2s; }
  .transcribing-dots .dot:nth-child(3) { animation-delay: 0.4s; }

  @keyframes dot-pulse {
    0%, 60%, 100% { opacity: 0; transform: translateY(0); }
    30% { opacity: 1; transform: translateY(-2px); }
  }

  .burst-ring {
    position: absolute;
    width: 80px;
    height: 80px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    pointer-events: none;
    opacity: 0;
    animation: burst-out 0.7s ease-out forwards;
  }

  .burst-ring-1 { animation-delay: 0s; }
  .burst-ring-2 { animation-delay: 0.08s; }
  .burst-ring-3 { animation-delay: 0.16s; }

  @keyframes burst-out {
    0% { transform: scale(1); opacity: 0.7; }
    100% { transform: scale(2.6); opacity: 0; }
  }

  .burst-flash {
    position: absolute;
    width: 80px;
    height: 80px;
    border-radius: 50%;
    background: radial-gradient(circle, color-mix(in srgb, var(--accent) 40%, transparent) 0%, transparent 70%);
    pointer-events: none;
    opacity: 0;
    animation: burst-flash 0.6s ease-out forwards;
  }

  @keyframes burst-flash {
    0% { transform: scale(1); opacity: 0.6; }
    100% { transform: scale(1.8); opacity: 0; }
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
    color: var(--text-secondary);
  }
</style>
