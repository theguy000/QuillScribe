# Tauri Parity TODO

1. Implement the recording overlay window equivalent to Tauri's `overlay.html` / `RecordingOverlay.svelte`.

2. Wire up auto-update support using the existing `self_update` dependency, including check, progress, and install behavior.

3. Add the tray `Use Model` submenu with dynamic API models, downloaded local models, checked active model state, and model switching.

4. Apply the always-on-top setting to the actual window instead of only persisting/logging it.

5. Persist overlay opacity changes from the UI and ensure the setting is applied wherever overlay rendering is implemented.

6. Persist and apply the maximum history entries setting from the UI.

7. Add Windows Start Menu/taskbar icon persistence so themed icons survive hide/show and shell refreshes.

8. Update tray menu state dynamically, especially Start/Stop Recording enabled states and menu rebuilds after settings/model changes.

9. Complete compositor fallback behavior for the recording overlay on non-composited Linux desktops.
