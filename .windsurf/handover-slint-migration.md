# QuillScribe Tauri → Slint Migration Handover

**Date:** 2026-05-03  
**Status:** Phase 1-4 + Build Fixes Complete, `cargo check` passes  
**Goal:** Reduce RAM from ~500MB (WebKitGTK) to ~15-40MB (Slint/FemtoVG)

---

## What Has Been Completed

### Phase 1: Project Scaffolding ✓
- [x] Updated `Cargo.toml` - removed Tauri deps, added Slint + helper crates
  - Added: `slint` (with `compat-1-2`, `backend-winit`, `renderer-femtovg`)
  - Added: `tray-icon`, `global-hotkey`, `auto-launch`, `self_update`, `notify-rust`
  - Added: `png` (for tray icon decoding)
  - Kept: All audio/whisper/clipboard/output modules
- [x] Updated `build.rs` - uses `slint_build::compile_with_config()`
- [x] Created Slint UI directory structure (`ui/`)
- [x] Removed frontend tooling (package.json, vite, svelte, src/, public/)
- [x] Removed `tauri.conf.json`

### Phase 2: Rust Backend Refactoring ✓
- [x] Rewrote `lib.rs` - Slint app bootstrap with callback wiring
- [x] Rewrote `commands.rs` - removed all Tauri commands, plain functions
- [x] Rewrote `tray.rs` - uses `tray-icon` crate, `Icon::from_rgba()` with `png` decoder, `thread_local!` for non-Sync `TrayIcon`
- [x] Rewrote `hotkey.rs` - uses `global-hotkey` crate
- [x] Rewrote `window.rs` - stubbed for Slint window management
- [x] `main.rs` unchanged (still calls `app_lib::run()`)

### Phase 3: Theme System ✓
- [x] Created `ui/themes/theme-definitions.slint` with all 12 themes
- [x] Defined `ThemeColors` struct with ~25 color properties
- [x] Global `Theme` with `get-theme(name)` function

### Phase 4: UI Component Stubs ✓
- [x] `app.slint` - Main window with callback declarations
- [x] `components/titlebar.slint` - Custom titlebar (window controls removed — Slint Window doesn't expose minimize/close)
- [x] `components/sidebar.slint` - Navigation sidebar with `SidebarButton` (defined before use)
- [x] `components/record-panel.slint` - Recording UI with mic button + `Dot` animation (defined before use)
- [x] `components/transcription-panel.slint` - Text display
- [x] `components/settings-dialog.slint` - Settings stub
- [x] `components/history-panel.slint` - History stub
- [x] `components/toast.slint` - Toast notifications

### Phase 7: Build & Packaging ✓
- [x] Updated `.github/workflows/ci.yml` - removed Node.js, added Slint check
- [x] Updated `PKGBUILD` - removed webkit2gtk, nodejs, npm deps
- [x] Build now uses pure `cargo build` instead of `npm install && npx tauri build`

### Build Error Fixes ✓
- [x] **Slint `background` property conflict** — Wrapped all component roots in `Rectangle` (Slint layouts don't have `background`)
- [x] **`root.window` not accessible** — Removed window minimize/close buttons from TitleBar (Slint Window doesn't expose these in `.slint` files; can be done via Rust/winit later)
- [x] **Component ordering** — Moved `SidebarButton` and `Dot` before their usage sites
- [x] **`pure callback` calling impure callbacks** — Changed all callbacks to `callback` (non-pure) since they trigger side effects
- [x] **`scale` property on Rectangle** — Replaced with `property <float> mic-scale` driving width/height/border-radius
- [x] **`vertical-alignment` on HorizontalLayout** — Moved to `Text` element instead
- [x] **`parent.width` not accessible** — Removed absolute positioning from ToastContainer
- [x] **`slint::WeakHandle`** → `slint::Weak` (correct Slint 1.x API)
- [x] **`AppWindow`** → `App` (Slint generates component from `export component App`)
- [x] **`tray_icon::icon::from_png_bytes`** — Replaced with `png::Decoder` + `Icon::from_rgba()`
- [x] **`TrayIcon` not `Send`/`Sync`** — Changed from `static Mutex` to `thread_local!`
- [x] **`save-settings({})` type mismatch** — Changed to `callback save-settings(string)`
- [x] **`get_recent_history(Some(7))`** — Changed to `get_recent_history(7)` (takes `i64`, not `Option`)

---

## Current State

**`cargo check` passes with 0 errors, ~55 warnings (mostly unused imports/variables).**

The app compiles but has not been runtime-tested yet.

---

## Remaining Work

### Immediate: Runtime Testing
1. **`cargo run`** — Verify the app actually launches and renders
2. **Test recording flow** — Toggle recording → transcription → output
3. **Test tray icon** — Verify tray appears and menu items work
4. **Test global hotkey** — Super+Shift+Space toggle
5. **Test theme switching** — Verify all 12 themes render correctly

### Phase 5: Complex Visual Effects (Pending)
- Wave ripple animation for mic button
- Dual spinner animation for transcribing state  
- Frost noise texture for overlay
- Multi-layer shadows
- CSS keyframe → Slint Timer animations

### Phase 6: Window Management & Integration (Pending)
- Always-on-top support (winit WindowLevel)
- Recording overlay window (separate winit window)
- Window minimize/close via Rust (winit API)
- Auto-start implementation (`auto-launch` crate)
- Update checking (`self_update` crate)
- System notifications (`notify-rust` crate)

### Phase 8: Testing & Verification (Pending)
- Compare visual fidelity with original
- RAM usage measurement
- Functional testing of all features

---

## Key Slint Gotchas Discovered

1. **Layout elements don't have `background`** — Must wrap in `Rectangle`
2. **`pure callback` cannot call impure callbacks** — Use `callback` for anything with side effects
3. **Non-exported components must be defined before use** — Order matters in `.slint` files
4. **`Window` doesn't expose `minimized`/`close()` in `.slint`** — Must use Rust-side winit API
5. **`parent.width` not accessible** — Use `root.width` or `self.width` from component scope
6. **`scale` is not a Rectangle property** — Use custom property + derived width/height
7. **`slint::WeakHandle` doesn't exist** — Use `slint::Weak<Component>`
8. **Slint generates `App` not `AppWindow`** — Matches the `export component App` name
9. **`tray_icon::TrayIcon` is not `Send`/`Sync`** — Use `thread_local!` instead of `static Mutex`

---

## Architecture Changes

### Before (Tauri)
```
Frontend (Svelte) ←→ Tauri IPC ←→ Rust Backend
     ↓                                  ↓
WebKitGTK (~500MB)              Audio/Whisper/Output
```

### After (Slint)
```
Slint UI (.slint files) ←→ Direct Rust Callbacks
     ↓                          ↓
FemtoVG (~15-40MB)      Audio/Whisper/Output
```

### Key Differences
- **No IPC layer** — Callbacks are direct Rust function calls
- **Single binary** — No web assets to bundle
- **Native rendering** — No WebView2/WebKitGTK dependency
- **Smaller memory footprint** — Native GPU rendering vs web engine

---

## File Structure Changes

### Deleted
```
package.json
package-lock.json
vite.config.js
svelte.config.js
jsconfig.json
index.html
overlay.html
src/ (entire Svelte frontend)
public/ (SVG assets)
src-tauri/tauri.conf.json
```

### Created
```
src-tauri/ui/
├── app.slint
├── themes/
│   └── theme-definitions.slint
├── components/
│   ├── titlebar.slint
│   ├── sidebar.slint
│   ├── record-panel.slint
│   ├── transcription-panel.slint
│   ├── settings-dialog.slint
│   ├── history-panel.slint
│   └── toast.slint
└── animations/ (empty - for future effects)
```

### Modified
```
src-tauri/Cargo.toml        # Dependencies updated (+png crate)
src-tauri/build.rs          # Slint build
src-tauri/src/lib.rs        # Slint app bootstrap (Weak<App>, fixed types)
src-tauri/src/commands.rs   # Removed Tauri commands
src-tauri/src/tray.rs       # tray-icon crate, Icon::from_rgba, thread_local!
src-tauri/src/hotkey.rs     # global-hotkey crate, Weak<App>
src-tauri/src/window.rs     # Stubbed for Slint
.github/workflows/ci.yml    # Removed Node.js
PKGBUILD                    # Removed webkit2gtk/nodejs
```

### Unchanged (Backend Logic)
```
src-tauri/src/audio.rs      # Audio capture
src-tauri/src/whisper.rs    # Transcription
src-tauri/src/config.rs     # Settings management
src-tauri/src/output.rs     # Clipboard/typing
src-tauri/src/sound.rs      # Notification sounds
src-tauri/src/statistics.rs # History management
```

---

## Next Steps for Completing Migration

1. **Runtime test** — `cargo run` and verify UI renders
2. **Implement remaining callbacks**
   - `save-settings` — parse JSON and save
   - `load-history` — feed data to UI
   - `check-for-update` — self_update integration
   - `install-update` — self_update install
3. **Add window controls** — minimize/close via Rust winit API
4. **Complete window management**
   - Always-on-top toggle
   - Overlay window for recording
   - Proper window state management
5. **Test and refine**
   - Full recording → transcription flow
   - Settings persistence
   - System tray functionality
   - Global hotkey
6. **Visual polish**
   - Implement animations
   - Fine-tune colors to match original
   - Test all 12 themes

---

## Known Limitations / TODOs

- **Window controls** — Minimize/close buttons removed from titlebar; need Rust-side winit implementation
- **Settings panel** — Currently a stub, needs full implementation
- **History panel** — Currently a stub, needs full implementation  
- **Update system** — Uses `self_update` but not fully wired
- **Overlay window** — Not yet implemented (was separate window in Tauri)
- **Auto-start** — `auto-launch` crate added but not integrated
- **Notifications** — `notify-rust` added but not integrated
- **Toast positioning** — Removed absolute positioning; needs proper overlay layout
- **55 compiler warnings** — Mostly unused imports/variables, should be cleaned up

---

## Testing Commands

```bash
# Check compilation (currently passes)
cd src-tauri && cargo check

# Build debug
cd src-tauri && cargo build

# Build release
cd src-tauri && cargo build --release

# Run (needs runtime testing)
cd src-tauri && cargo run
```

---

## Resources

- **Slint Docs:** https://slint.dev/docs/
- **Migration Plan:** `.windsurf/plans/quillscribe-slint-migration-19547a.md`
- **Original Tauri Code:** See git history for reference
