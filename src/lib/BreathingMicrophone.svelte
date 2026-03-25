<script>
	/** @type {{ isRecording?: boolean, audioLevel?: number, showWaveform?: boolean, isDark?: boolean, accentColor?: string, onclick?: () => void }} */
	let {
		isRecording = false,
		audioLevel = 0,
		showWaveform = true,
		isDark = false,
		accentColor = '#4A90E2',
		onclick
	} = $props();

	/** @type {HTMLCanvasElement} */
	let canvas;

	let animFrameId = $state(0);
	let phase1 = $state(0);
	let phase2 = $state(0);
	let phase3 = $state(0);
	let transitionProgress = $state(0);
	let lastTimestamp = $state(0);

	const WIDTH = 200;
	const HEIGHT = 200;
	const CENTER_X = WIDTH / 2;
	const CENTER_Y = HEIGHT / 2;
	const NUM_POINTS = 90;

	const ribbons = [
		{ color: [0, 255, 255], alpha: 0.4, speed: 1.0, freqs: [3, 5, 7], amps: [0.15, 0.1, 0.06] },
		{ color: [0, 122, 255], alpha: 0.6, speed: 1.2, freqs: [4, 6, 9], amps: [0.12, 0.09, 0.07] },
		{ color: [175, 82, 222], alpha: 0.3, speed: 0.8, freqs: [2, 5, 8], amps: [0.18, 0.08, 0.05] }
	];

	/**
	 * Parse a hex color string to [r, g, b].
	 * @param {string} hex
	 * @returns {number[]}
	 */
	function hexToRgb(hex) {
		const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
		return result
			? [parseInt(result[1], 16), parseInt(result[2], 16), parseInt(result[3], 16)]
			: [74, 144, 226];
	}

	/**
	 * Draw the microphone icon on the canvas context.
	 * @param {CanvasRenderingContext2D} ctx
	 * @param {string} color
	 * @param {number} scale
	 */
	function drawMicrophone(ctx, color, scale) {
		ctx.save();
		ctx.translate(CENTER_X, CENTER_Y);
		ctx.scale(scale, scale);

		// Mic capsule (rounded rectangle)
		const capsuleW = 20;
		const capsuleH = 32;
		const capsuleR = capsuleW / 2;
		const capsuleTop = -28;

		ctx.beginPath();
		ctx.moveTo(-capsuleW / 2, capsuleTop + capsuleR);
		ctx.arcTo(-capsuleW / 2, capsuleTop, 0, capsuleTop, capsuleR);
		ctx.arcTo(capsuleW / 2, capsuleTop, capsuleW / 2, capsuleTop + capsuleR, capsuleR);
		ctx.lineTo(capsuleW / 2, capsuleTop + capsuleH - capsuleR);
		ctx.arcTo(capsuleW / 2, capsuleTop + capsuleH, 0, capsuleTop + capsuleH, capsuleR);
		ctx.arcTo(-capsuleW / 2, capsuleTop + capsuleH, -capsuleW / 2, capsuleTop + capsuleH - capsuleR, capsuleR);
		ctx.closePath();
		ctx.fillStyle = color;
		ctx.fill();

		// Mic stand arc
		const arcRadius = 16;
		const arcCenterY = capsuleTop + capsuleH - 2;
		ctx.beginPath();
		ctx.arc(0, arcCenterY, arcRadius, 0, Math.PI, false);
		ctx.strokeStyle = color;
		ctx.lineWidth = 2.5;
		ctx.lineCap = 'round';
		ctx.stroke();

		// Stand line
		const standTop = arcCenterY + arcRadius;
		const standBottom = standTop + 10;
		ctx.beginPath();
		ctx.moveTo(0, standTop);
		ctx.lineTo(0, standBottom);
		ctx.stroke();

		// Base line
		ctx.beginPath();
		ctx.moveTo(-10, standBottom);
		ctx.lineTo(10, standBottom);
		ctx.stroke();

		ctx.restore();
	}

	/**
	 * Draw a single ribbon waveform.
	 * @param {CanvasRenderingContext2D} ctx
	 * @param {number} phase
	 * @param {{ color: number[], alpha: number, speed: number, freqs: number[], amps: number[] }} ribbon
	 * @param {number} level
	 * @param {number} transition
	 */
	function drawRibbon(ctx, phase, ribbon, level, transition) {
		if (transition <= 0.001) return;

		const baseRadius = 55;
		const effectiveAlpha = ribbon.alpha * transition;
		const amplitudeScale = 1.0 + level * 2.5;

		ctx.beginPath();

		for (let i = 0; i <= NUM_POINTS; i++) {
			const angle = (i / NUM_POINTS) * Math.PI * 2;

			let radiusMod = 0;
			for (let f = 0; f < ribbon.freqs.length; f++) {
				radiusMod += Math.sin(angle * ribbon.freqs[f] + phase * (f + 1) * 0.7) * ribbon.amps[f] * amplitudeScale;
			}

			// Add audio-reactive high-frequency detail
			radiusMod += Math.sin(angle * 12 + phase * 3) * level * 0.08;
			radiusMod += Math.sin(angle * 18 + phase * 5) * level * 0.04;

			const radius = baseRadius * (1 + radiusMod);
			const x = CENTER_X + Math.cos(angle) * radius;
			const y = CENTER_Y + Math.sin(angle) * radius;

			if (i === 0) {
				ctx.moveTo(x, y);
			} else {
				// Use quadratic curves for smoother shapes
				const prevAngle = ((i - 0.5) / NUM_POINTS) * Math.PI * 2;
				let prevRadiusMod = 0;
				for (let f = 0; f < ribbon.freqs.length; f++) {
					prevRadiusMod += Math.sin(prevAngle * ribbon.freqs[f] + phase * (f + 1) * 0.7) * ribbon.amps[f] * amplitudeScale;
				}
				prevRadiusMod += Math.sin(prevAngle * 12 + phase * 3) * level * 0.08;
				prevRadiusMod += Math.sin(prevAngle * 18 + phase * 5) * level * 0.04;
				const cpRadius = baseRadius * (1 + prevRadiusMod);
				const cpx = CENTER_X + Math.cos(prevAngle) * cpRadius;
				const cpy = CENTER_Y + Math.sin(prevAngle) * cpRadius;
				ctx.quadraticCurveTo(cpx, cpy, x, y);
			}
		}

		ctx.closePath();

		// Gradient fill for richer look
		const gradient = ctx.createRadialGradient(CENTER_X, CENTER_Y, 20, CENTER_X, CENTER_Y, baseRadius * 1.3);
		const [r, g, b] = ribbon.color;
		gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${effectiveAlpha * 0.3})`);
		gradient.addColorStop(0.6, `rgba(${r}, ${g}, ${b}, ${effectiveAlpha * 0.7})`);
		gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, ${effectiveAlpha * 0.1})`);

		ctx.fillStyle = gradient;
		ctx.fill();

		// Ribbon edge glow
		ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${effectiveAlpha * 0.5})`;
		ctx.lineWidth = 1;
		ctx.stroke();
	}

	/**
	 * Draw the idle background circle.
	 * @param {CanvasRenderingContext2D} ctx
	 * @param {number[]} rgb
	 * @param {number} opacity
	 */
	function drawIdleCircle(ctx, rgb, opacity) {
		if (opacity <= 0.001) return;

		const [r, g, b] = rgb;
		const gradient = ctx.createRadialGradient(CENTER_X, CENTER_Y, 20, CENTER_X, CENTER_Y, 65);
		gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${opacity * 0.2})`);
		gradient.addColorStop(0.7, `rgba(${r}, ${g}, ${b}, ${opacity * 0.1})`);
		gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

		ctx.beginPath();
		ctx.arc(CENTER_X, CENTER_Y, 65, 0, Math.PI * 2);
		ctx.fillStyle = gradient;
		ctx.fill();

		// Subtle ring
		ctx.beginPath();
		ctx.arc(CENTER_X, CENTER_Y, 55, 0, Math.PI * 2);
		ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${opacity * 0.25})`;
		ctx.lineWidth = 1.5;
		ctx.stroke();
	}

	/**
	 * Main render loop.
	 * @param {number} timestamp
	 */
	function render(timestamp) {
		if (!canvas) return;

		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const dpr = window.devicePixelRatio || 1;
		if (canvas.width !== WIDTH * dpr || canvas.height !== HEIGHT * dpr) {
			canvas.width = WIDTH * dpr;
			canvas.height = HEIGHT * dpr;
			ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		}

		// Delta time
		const dt = lastTimestamp ? (timestamp - lastTimestamp) / 16.667 : 1; // normalized to 60fps
		lastTimestamp = timestamp;

		// Transition progress
		const transitionSpeed = 0.06 * dt;
		if (isRecording) {
			transitionProgress = Math.min(1, transitionProgress + transitionSpeed);
		} else {
			transitionProgress = Math.max(0, transitionProgress - transitionSpeed);
		}

		// Ease the transition
		const easedTransition = transitionProgress < 0.5
			? 2 * transitionProgress * transitionProgress
			: 1 - Math.pow(-2 * transitionProgress + 2, 2) / 2;

		// Advance phases
		const baseAdvance = 0.05 * dt;
		const reactivity = audioLevel * 0.1 * dt;
		phase1 += baseAdvance * ribbons[0].speed + reactivity;
		phase2 += baseAdvance * ribbons[1].speed + reactivity;
		phase3 += baseAdvance * ribbons[2].speed + reactivity;

		// Clear
		ctx.clearRect(0, 0, WIDTH, HEIGHT);

		// Drop shadow setup
		ctx.shadowColor = isDark ? 'rgba(0, 0, 0, 0.5)' : 'rgba(0, 0, 0, 0.2)';
		ctx.shadowBlur = 15;
		ctx.shadowOffsetX = 0;
		ctx.shadowOffsetY = 4;

		const accentRgb = hexToRgb(accentColor);

		// Draw idle circle (fades out when recording)
		drawIdleCircle(ctx, accentRgb, 1 - easedTransition);

		// Reset shadow for ribbons (they have their own glow)
		ctx.shadowColor = 'transparent';
		ctx.shadowBlur = 0;
		ctx.shadowOffsetX = 0;
		ctx.shadowOffsetY = 0;

		// Draw ribbons when recording or transitioning
		if (showWaveform && easedTransition > 0.001) {
			// Draw in order: back to front
			drawRibbon(ctx, phase3, ribbons[2], audioLevel, easedTransition);
			drawRibbon(ctx, phase1, ribbons[0], audioLevel, easedTransition);
			drawRibbon(ctx, phase2, ribbons[1], audioLevel, easedTransition);
		}

		// Mic icon color interpolation
		const micColor = isRecording || easedTransition > 0.5 ? lerpColor(accentRgb, [255, 255, 255], easedTransition) : accentColor;

		// Breathing scale for idle state
		const breatheScale = 1.0 + Math.sin(timestamp / 800) * 0.02 * (1 - easedTransition);
		const recordScale = 1.0 + audioLevel * 0.08 * easedTransition;
		const micScale = breatheScale * recordScale;

		// Re-enable shadow for mic
		ctx.shadowColor = isDark ? 'rgba(0, 0, 0, 0.4)' : 'rgba(0, 0, 0, 0.15)';
		ctx.shadowBlur = 8;
		ctx.shadowOffsetY = 2;

		drawMicrophone(ctx, typeof micColor === 'string' ? micColor : `rgb(${micColor[0]}, ${micColor[1]}, ${micColor[2]})`, micScale);

		ctx.shadowColor = 'transparent';
		ctx.shadowBlur = 0;
		ctx.shadowOffsetY = 0;

		animFrameId = requestAnimationFrame(render);
	}

	/**
	 * Linearly interpolate between two RGB colors.
	 * @param {number[]} from
	 * @param {number[]} to
	 * @param {number} t
	 * @returns {number[]}
	 */
	function lerpColor(from, to, t) {
		return [
			Math.round(from[0] + (to[0] - from[0]) * t),
			Math.round(from[1] + (to[1] - from[1]) * t),
			Math.round(from[2] + (to[2] - from[2]) * t)
		];
	}

	$effect(() => {
		// Start animation loop when canvas is available
		if (canvas) {
			lastTimestamp = 0;
			animFrameId = requestAnimationFrame(render);

			return () => {
				if (animFrameId) {
					cancelAnimationFrame(animFrameId);
				}
			};
		}
	});
</script>

<button
	class="breathing-microphone"
	class:is-dark={isDark}
	class:is-recording={isRecording}
	{onclick}
	aria-label={isRecording ? 'Stop recording' : 'Start recording'}
	type="button"
>
	<canvas
		bind:this={canvas}
		width={200}
		height={200}
		style="width: 200px; height: 200px;"
	></canvas>
</button>

<style>
	.breathing-microphone {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 200px;
		height: 200px;
		padding: 0;
		margin: 0;
		border: none;
		background: transparent;
		cursor: pointer;
		border-radius: 50%;
		outline: none;
		position: relative;
		transition: transform 150ms ease;
		-webkit-tap-highlight-color: transparent;
	}

	.breathing-microphone:hover {
		transform: scale(1.04);
	}

	.breathing-microphone:active {
		transform: scale(0.97);
	}

	.breathing-microphone:focus-visible {
		box-shadow: 0 0 0 3px rgba(74, 144, 226, 0.5);
	}

	.breathing-microphone canvas {
		display: block;
		pointer-events: none;
	}
</style>
