# QuillScribe: Tauri → Slint Migration Plan

Replace the Tauri (WebKitGTK) UI stack with Slint to reduce RAM from ~500-600MB to ~15-40MB, retaining all current visual fidelity and functionality.

## Approach: Full rewrite in-place

The existing Tauri frontend (Svelte + WebKitGTK) is replaced with Slint UI in the same repo. The Rust backend modules stay almost identical — only the IPC/command layer changes. The app will not build during transition.

---

## Phase 1: Project Scaffolding

### 1.1 Update `Cargo.toml`
- Remove: `tauri`, `tauri-plugin-global-shortcut`, `tauri-plugin-autostart`, `tauri-plugin-updater`, `tauri-plugin-dialog`, `tauri-plugin-fs`, `tauri-plugin-process`, `tauri-plugin-notification`
- Add: `slint` (with `backend-winit` + `renderer-femtovg` features), `slint-build`, `i-slint-backend-winit`
- Add: `tray-icon` crate (for system tray — Slint doesn't include one), `global-hotkey` crate
- Keep: `whisper-rs`, `cpal`, `hound`, `reqwest`, `tokio`, `serde`, `serde_json`, `arboard`, `rodio`, `dirs`, `chrono`, `log`, `env_logger`, `anyhow`, `rand`

### 1.2 Update `build.rs`
- Replace `tauri_build::build()` with `slint_build::compile_with_config()` pointing to the main `.slint` file
- Configure Slint include paths and style

### 1.3 Remove frontend tooling
- Delete: `package.json`, `package-lock.json`, `vite.config.js`, `svelte.config.js`, `jsconfig.json`, `index.html`, `overlay.html`
- Delete: `src/` directory (Svelte components, `app.css`, `main.js`, `overlay.js`)
- Delete: `public/` directory (SVG assets — will be embedded or replaced in Slint)
- Remove Node.js from CI/CD workflows

### 1.4 Create Slint UI directory structure
```
src-tauri/ui/
├── app.slint              # Main app layout
├── components/
│   ├── titlebar.slint
│   ├── sidebar.slint
│   ├── record-panel.slint
│   ├── transcription-panel.slint
│   ├── settings-dialog.slint
│   ├── history-panel.slint
│   ├── toast.slint
│   ├── custom-select.slint
│   ├── quill-icon.slint
│   └── overlay/
│       ├── minimal-overlay.slint
│       └── full-overlay.slint
├── themes/
│   └── theme-definitions.slint  # All 12 theme color palettes
└── animations/
    └── effects.slint       # Shared animation helpers
```

---

## Phase 2: Rust Backend Refactoring

### 2.1 Remove Tauri command layer
- Delete `src-tauri/src/commands.rs` — all `#[tauri::command]` functions
- The logic they call (in `audio.rs`, `whisper.rs`, `config.rs`, etc.) stays, but is called directly from Slint callbacks instead of through IPC

### 2.2 Refactor `lib.rs`
- Remove Tauri builder (`tauri::Builder`)
- Remove `on_window_event` handler for overlay
- Remove WebKitGTK DMA-BUF workaround
- Replace with Slint app setup: create main window, set up Slint component, wire callbacks

### 2.3 Refactor `tray.rs`
- Replace `tauri::tray` with `tray-icon` crate
- Same menu structure (Show, Start/Stop Recording, Use Model submenu, Settings, Exit)
- Same theme-based icon selection using `include_bytes!`
- Emit events to Slint component instead of Tauri `app.emit()`

### 2.4 Refactor `hotkey.rs`
- Replace `tauri_plugin_global_shortcut` with `global-hotkey` crate
- Same shortcut string parsing and conversion logic
- Callback triggers Slint component callback instead of Tauri event

### 2.5 Refactor `window.rs`
- Remove Tauri `WebviewWindow` API calls
- Use Slint/winit window API for always-on-top, custom titlebar, etc.

### 2.6 Refactor `main.rs`
- Replace `app_lib::run()` with Slint application launch
- Initialize Slint component, wire up all Rust callbacks, start event loop

### 2.7 Keep unchanged
- `audio.rs` — audio capture, monitoring, recording (no Tauri deps)
- `whisper.rs` — transcription logic (no Tauri deps)
- `config.rs` — settings management (no Tauri deps)
- `output.rs` — clipboard/typing output (no Tauri deps)
- `sound.rs` — notification sounds (no Tauri deps)
- `statistics.rs` — history management (no Tauri deps)

---

## Phase 3: Theme System in Slint

### 3.1 Define theme structs in Slint
- Create a `ThemeColors` struct in `theme-definitions.slint` with all ~25 color properties from `app.css`
- Define all 12 themes as `global` constants: white, warm_gray, soft_beige, blue_gray, warm_taupe, soft_sage, dark_charcoal, dark_blue, dark_purple, dark_forest, dark_burgundy, obsidian

### 3.2 Implement `color-mix()` equivalent in Rust
- Create a Rust function `color_mix(color1: Color, color2: Color, ratio: f32) -> Color`
- Expose this as a Slint callback so the UI can request blended colors
- Precompute common blends at theme-switch time for performance

### 3.3 Theme switching
- When theme changes, Rust callback updates the `ThemeColors` struct property on the Slint component
- Slint's reactive binding automatically propagates to all child components

---

## Phase 4: UI Components (in order of implementation)

### 4.1 TitleBar
- Custom titlebar with drag region, icon, title, minimize/close buttons
- Slint `TouchArea` for drag, `clicked` callbacks for buttons
- Background gradient via Rust-computed colors

### 4.2 Sidebar
- Logo, navigation buttons (Record, Settings, History), user section
- SVG icons rendered as Slint `Image` elements (convert SVGs to PNG at build time or use Slint's SVG support)
- Active state highlighting, update notification dot with pulse animation

### 4.3 RecordPanel
- **Mic button**: 128×128 `Rectangle` with border-radius, SVG mic/stop icon overlay
- **Wave ripple animation**: 3 concentric `Rectangle` circles with `animate opacity` + `animate scale`, driven by `Timer` with staggered intervals (0s, 0.8s, 1.6s)
- **Dual spinner**: Two `Rectangle` arcs drawn via `Canvas` element, rotated by `Timer` at different speeds
- **Burst effect**: 3 `Rectangle` rings + radial gradient flash, triggered on stop, animated via `Timer`
- **Audio level scaling**: Bind mic-ring `scale` property to `audio_level` callback from Rust
- **Transcribing dots**: 3 `Text` elements with staggered `animate opacity` via `Timer`

### 4.4 TranscriptionPanel
- Transcription text display with copy/edit buttons
- Edit mode: `TextInput` multiline
- Empty state placeholder
- Scrolling via Slint `ListView` or `Flickable`

### 4.5 CustomSelect
- Reusable dropdown component
- Trigger button with chevron, dropdown list with keyboard navigation
- `TouchArea` for click-outside-to-close
- Position dropdown using `init` callback to get parent position

### 4.6 SettingsDialog
- Tab-based layout (Audio, Engine, UI, Output, Keyboard, About)
- Reuse `CustomSelect` for dropdowns
- Radio groups, checkboxes, sliders, stepper controls
- Mic test with live audio level bar
- Model download/delete with progress indicator
- Keyboard shortcut recording input
- About tab with version, update check, release notes

### 4.7 HistoryPanel
- Scrollable list of history entries
- Compact/detailed view toggle
- Expandable entries with transcription text
- Copy button per entry
- Empty/loading/error states

### 4.8 Toast
- Position at bottom-center of window
- Animate in: `animate opacity` + `animate y` (slide up + fade in)
- Auto-dismiss via `Timer`
- Success/error/info variants with icon + color

### 4.9 Recording Overlay (separate window)
- **Minimal mode**: Rounded pill (120×32), audio level bars (16 thin `Rectangle` elements with animated heights)
- **Full mode**: Wider pill (240×48), REC indicator, elapsed timer, stop button, 20 audio bars
- **Overlay styles**: All 7 styles from `overlayStyles.js`:
  - Flat, Frosted Glass, Subtle Gradient, Gradient Theme, Neon Glow, Gradient + Glass, Neumorphism
- **Frost noise texture**: Use Slint `Canvas` element to procedurally generate noise from Rust, composite over background
- **Neon glow breathing**: `Timer`-driven `animate` on `drop-shadow-blur` property
- **Transparency**: Use FemtoVG renderer with `visible: false` initially, show on demand
- **No-compositor fallback**: Detect via Rust (check for compositor), set `border-radius: 0` and opaque background

---

## Phase 5: Complex Visual Effects (full fidelity)

### 5.1 `color-mix()` gradients
- All CSS `color-mix(in srgb, ...)` calls are replaced with Rust-computed `LinearGradient` stops
- Rust function receives theme colors + blend ratios, returns array of gradient stops
- Slint `LinearGradient` element uses these stops directly

### 5.2 Multi-layer box-shadow
- CSS `box-shadow` with multiple layers → Slint `drop-shadow` is single-layer
- **Solution**: Stack multiple `Rectangle` elements behind the target, each with its own `drop-shadow` at different offsets/blur/color
- This replicates the multi-layer shadow effect exactly

### 5.3 CSS keyframe animations → Slint Timer-driven animations
- Each `@keyframes` animation becomes a `Timer` + property binding:
  - `wave-ripple`: 3 `Timer`s at 2.4s period, staggered 0.8s, driving `opacity` and `scale` on 3 ring elements
  - `spin-slow` / `spin-fast`: `Timer` at 16ms interval, incrementing `rotation-angle` property on arc elements
  - `burst-out`: One-shot `Timer` driving `opacity` + `scale` on 3 ring elements with 80ms stagger
  - `neon-breathe`: `Timer` at 50ms interval, sinusoidal modulation of shadow blur
  - `dot-pulse` / `dot-blink`: `Timer` driving `opacity` on indicator elements
  - `toast-in`: `animate opacity` + `animate y` on toast container

### 5.4 Frost SVG filter → Canvas procedural noise
- The `feTurbulence` + `feColorMatrix` + `feBlend` pipeline cannot be done in Slint's declarative markup
- **Solution**: Use Slint's `Canvas` element with a Rust callback that:
  1. Generates fractal noise using a simple Perlin noise implementation
  2. Desaturates it
  3. Composites it over the background using overlay blend mode
  4. Returns the resulting pixel buffer as a Slint `Image`
- The noise texture is generated once when the overlay style is set, not every frame
- Cache the texture in Rust to avoid recomputation

### 5.5 Dual SVG spinner → Canvas arcs
- The two concentric spinning arcs are drawn via `Canvas` element
- Rust callback draws two arc paths with different dash patterns
- `Timer` increments rotation angles at different speeds (2s and 1.2s period)

---

## Phase 6: Window Management & Integration

### 6.1 Main window
- Create via Slint/winit with custom titlebar
- Always-on-top support via winit `WindowLevel`
- Min size 800×480, default 960×560
- No native decorations (custom titlebar)

### 6.2 Overlay window
- Created lazily when recording starts and main window loses focus
- Separate Slint component in a second winit window
- Transparent background (FemtoVG renderer supports this)
- Always-on-top, skip-taskbar, non-focusable
- Position at bottom-center of current monitor (winit API)
- Destroyed (not hidden) when recording stops or main window regains focus
- This eliminates the second WebKitGTK instance entirely when not recording

### 6.3 System tray
- `tray-icon` crate for cross-platform tray icon
- Same menu structure as current `tray.rs`
- Themed icon PNGs loaded via `include_bytes!`
- Menu events trigger Slint callbacks

### 6.4 Global hotkey
- `global-hotkey` crate for cross-platform shortcut registration
- Same shortcut string format and conversion logic
- Hotkey press triggers Slint callback to toggle recording

### 6.5 Auto-start, updates, notifications
- **Auto-start**: Use `auto-launch` crate instead of `tauri-plugin-autostart`
- **Updates**: Use `self_update` crate instead of `tauri-plugin-updater`
- **Notifications**: Use `notify-rust` crate instead of `tauri-plugin-notification`

---

## Phase 7: Build & Packaging

### 7.1 Update CI/CD workflows
- Remove Node.js install steps from `.github/workflows/ci.yml`
- Remove `npm install` and `npm run build` steps
- Cargo build now produces the standalone binary directly
- No web asset bundling needed

### 7.2 Update packaging
- **AUR (PKGBUILD, .SRCINFO)**: Remove node/npm dependencies, update build commands to pure `cargo build`
- **AppImage**: Update to bundle the Slint binary + required shared libs (Vulkan/Mesa for FemtoVG)
- **Windows**: Update installer to bundle the Slint binary (no WebView2 bootstrapper needed)
- **install.sh**: Remove Node.js dependency check

### 7.3 FemtoVG renderer requirements
- Linux: Requires Vulkan or OpenGL ES 2.0 (available on virtually all modern systems)
- Windows: Uses Direct3D or Vulkan via wgpu
- macOS: Uses Metal via wgpu
- Fallback: Software renderer if no GPU available (slower but functional)

---

## Phase 8: Testing & Verification

### 8.1 Functional testing
- Verify all 12 themes render correctly
- Verify all 7 overlay styles render correctly
- Verify recording → transcription → output flow works
- Verify settings persistence
- Verify history panel
- Verify global hotkey
- Verify system tray
- Verify auto-start
- Verify update checking

### 8.2 Visual fidelity testing
- Compare side-by-side screenshots of Tauri vs Slint versions
- Verify animations match timing and appearance
- Verify frost noise texture looks correct
- Verify gradient blends match `color-mix()` output
- Verify multi-layer shadows match CSS `box-shadow`

### 8.3 RAM measurement
- Measure RSS on Linux: `ps aux | grep quillscribe`
- Target: <50MB idle, <100MB during recording with overlay visible
- Compare with current ~500-600MB

---

## File Change Summary

### Delete
- `package.json`, `package-lock.json`, `vite.config.js`, `svelte.config.js`, `jsconfig.json`
- `index.html`, `overlay.html`
- `src/` (entire directory — Svelte app)
- `public/` (SVG assets — replaced by Slint equivalents)
- `src-tauri/tauri.conf.json`
- `src-tauri/src/commands.rs` (Tauri IPC layer)

### Create
- `src-tauri/ui/` (all `.slint` UI files)
- `src-tauri/src/app.rs` (Slint app setup, callback wiring)
- `src-tauri/src/theme.rs` (color-mix, gradient computation, noise generation)

### Modify heavily
- `src-tauri/Cargo.toml` (swap Tauri for Slint + helper crates)
- `src-tauri/build.rs` (Slint build instead of Tauri build)
- `src-tauri/src/main.rs` (Slint event loop)
- `src-tauri/src/lib.rs` (remove Tauri builder, add Slint setup)
- `src-tauri/src/tray.rs` (use `tray-icon` crate)
- `src-tauri/src/hotkey.rs` (use `global-hotkey` crate)
- `src-tauri/src/window.rs` (use winit API)
- `.github/workflows/ci.yml` (remove Node.js steps)
- `PKGBUILD`, `.SRCINFO` (remove Node.js deps)
- `install.sh` (remove Node.js check)

### Keep unchanged
- `src-tauri/src/audio.rs`
- `src-tauri/src/whisper.rs`
- `src-tauri/src/config.rs`
- `src-tauri/src/output.rs`
- `src-tauri/src/sound.rs`
- `src-tauri/src/statistics.rs`
- `src-tauri/sounds/` (embedded WAV files)
- `src-tauri/icons/` (tray/taskbar PNGs — still used)

---

## Execution Order

1. **Phase 1** — Scaffolding (can't build yet, but structure is ready)
2. **Phase 2** — Backend refactoring (remove Tauri deps from Rust)
3. **Phase 3** — Theme system (foundation for all UI)
4. **Phase 4.1–4.3** — TitleBar, Sidebar, RecordPanel (main shell is functional)
5. **Phase 4.4–4.8** — TranscriptionPanel, CustomSelect, Settings, History, Toast
6. **Phase 4.9** — Overlay window
7. **Phase 5** — Complex visual effects (frost, shadows, animations)
8. **Phase 6** — Window management & integration (tray, hotkey, auto-start, updates)
9. **Phase 7** — Build & packaging
10. **Phase 8** — Testing & verification

Each phase builds on the previous. The app becomes compilable after Phase 4.3 (basic shell works). Full feature parity is achieved after Phase 6.
