/**
 * Overlay visual style definitions — single source of truth.
 *
 * Each style provides a factory function that receives an opacity value
 * (0–1) and returns CSS property maps for light and dark themes.
 * Glass-based styles use the opacity parameter for their background alpha;
 * opaque styles ignore it.
 */

/** @typedef {{ background: string, border: string, boxShadow: string, backdropFilter?: string }} OverlayCSSProps */
/** @typedef {{ value: string, label: string, supportsOpacity: boolean, hasFrost?: boolean, light: (opacity: number) => OverlayCSSProps, dark: (opacity: number) => OverlayCSSProps }} OverlayStyleDef */

/** @type {OverlayStyleDef[]} */
export const overlayStyles = [
  {
    value: 'default',
    label: 'Flat',
    supportsOpacity: false,
    light: () => ({
      background: 'var(--bg-secondary, #eef2f7)',
      border: '1px solid color-mix(in srgb, var(--border) 60%, transparent)',
      boxShadow: '0 2px 12px var(--shadow)',
    }),
    dark: () => ({
      background: 'var(--bg-secondary, #1e2230)',
      border: '1px solid color-mix(in srgb, var(--border) 60%, transparent)',
      boxShadow: '0 2px 12px var(--shadow)',
    }),
  },
  {
    value: 'frosted_glass',
    label: 'Frosted Glass',
    supportsOpacity: true,
    // Simulated frost: milky layered gradient + noise overlay (applied via CSS class).
    // backdrop-filter does not work in a standalone transparent Tauri webview,
    // so we fake the frosted look with opaque-ish gradients and inset glow.
    hasFrost: true,
    light: (o) => ({
      background: `linear-gradient(160deg, rgba(240,244,255,${o}) 0%, rgba(230,235,248,${Math.max(0, o - 0.04)}) 40%, rgba(220,228,245,${Math.max(0, o - 0.08)}) 100%)`,
      border: `1px solid rgba(255, 255, 255, ${Math.min(1, o * 0.8)})`,
      boxShadow: `0 4px 20px rgba(0,0,0,0.06), inset 0 1px 0 rgba(255,255,255,0.9), inset 0 -1px 0 rgba(200,210,230,0.25)`,
    }),
    dark: (o) => ({
      background: `linear-gradient(160deg, rgba(32,36,52,${o}) 0%, rgba(28,32,48,${Math.max(0, o - 0.04)}) 40%, rgba(24,28,44,${Math.max(0, o - 0.08)}) 100%)`,
      border: '1px solid rgba(255, 255, 255, 0.1)',
      boxShadow: `0 4px 20px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.06), inset 0 -1px 0 rgba(0,0,0,0.2)`,
    }),
  },
  {
    value: 'subtle_gradient',
    label: 'Subtle Gradient',
    supportsOpacity: false,
    light: () => ({
      background: 'linear-gradient(135deg, #dce4ff 0%, #f0ecf8 40%, #f8e8ef 100%)',
      border: '1px solid rgba(180, 175, 210, 0.35)',
      boxShadow: '0 4px 16px rgba(0,0,0,0.06), inset 0 1px 0 rgba(255,255,255,0.8)',
    }),
    dark: () => ({
      background: 'linear-gradient(135deg, #1a2240 0%, #28223a 40%, #2e1e2a 100%)',
      border: '1px solid rgba(255, 255, 255, 0.08)',
      boxShadow: '0 4px 16px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.04)',
    }),
  },
  {
    value: 'gradient_theme',
    label: 'Gradient Theme',
    supportsOpacity: false,
    light: () => ({
      background: 'linear-gradient(135deg, color-mix(in srgb, var(--accent) 30%, var(--bg-secondary)) 0%, var(--bg-secondary) 50%, color-mix(in srgb, var(--accent-hover) 25%, var(--bg-tertiary)) 100%)',
      border: '1px solid color-mix(in srgb, var(--accent) 25%, var(--border-light))',
      boxShadow: '0 4px 16px color-mix(in srgb, var(--accent-glow) 40%, var(--shadow)), inset 0 1px 0 color-mix(in srgb, var(--accent) 8%, rgba(255,255,255,0.7))',
    }),
    dark: () => ({
      background: 'linear-gradient(135deg, color-mix(in srgb, var(--accent) 25%, var(--bg-secondary)) 0%, var(--bg-secondary) 50%, color-mix(in srgb, var(--accent-hover) 20%, var(--bg-tertiary)) 100%)',
      border: '1px solid color-mix(in srgb, var(--accent) 22%, var(--border))',
      boxShadow: '0 4px 16px color-mix(in srgb, var(--accent-glow) 30%, var(--shadow-lg)), inset 0 1px 0 color-mix(in srgb, var(--accent) 8%, rgba(255,255,255,0.06))',
    }),
  },
  {
    value: 'neon_glow',
    label: 'Neon Glow',
    supportsOpacity: false,
    light: () => ({
      background: 'var(--bg-secondary, #eef2f7)',
      border: '1.5px solid rgba(37, 99, 235, 0.35)',
      boxShadow: '0 0 14px rgba(37,99,235,0.15), 0 0 30px rgba(37,99,235,0.06), inset 0 0 10px rgba(37,99,235,0.04)',
    }),
    dark: () => ({
      background: 'var(--bg-secondary, #161824)',
      border: '1.5px solid rgba(37, 99, 235, 0.45)',
      boxShadow: '0 0 14px rgba(37,99,235,0.25), 0 0 30px rgba(37,99,235,0.1), inset 0 0 10px rgba(37,99,235,0.06)',
    }),
  },
  {
    value: 'gradient_glass',
    label: 'Gradient + Glass',
    supportsOpacity: true,
    hasFrost: true,
    light: (o) => ({
      background: `linear-gradient(135deg, rgba(210,220,255,${o}) 0%, rgba(240,238,250,${Math.max(0, o - 0.05)}) 45%, rgba(225,215,248,${Math.max(0, o - 0.03)}) 100%)`,
      border: '1px solid rgba(180, 190, 230, 0.4)',
      boxShadow: '0 4px 24px rgba(100,80,200,0.08), inset 0 1px 0 rgba(255,255,255,0.85), inset 0 -1px 0 rgba(180,170,220,0.2)',
    }),
    dark: (o) => ({
      background: `linear-gradient(135deg, rgba(35,30,60,${o}) 0%, rgba(25,28,48,${Math.max(0, o - 0.05)}) 45%, rgba(20,35,52,${Math.max(0, o - 0.03)}) 100%)`,
      border: '1px solid rgba(120, 100, 200, 0.18)',
      boxShadow: '0 4px 24px rgba(0,0,0,0.3), inset 0 1px 0 rgba(160,140,220,0.1), inset 0 -1px 0 rgba(0,0,0,0.15)',
    }),
  },
  {
    value: 'neumorphism',
    label: 'Neumorphism',
    supportsOpacity: false,
    light: () => ({
      background: '#e4e9f0',
      border: 'none',
      boxShadow: '5px 5px 12px rgba(0,0,0,0.08), -5px -5px 12px rgba(255,255,255,0.9)',
    }),
    dark: () => ({
      background: '#1e2230',
      border: 'none',
      boxShadow: '5px 5px 12px rgba(0,0,0,0.35), -5px -5px 12px rgba(60,65,85,0.18)',
    }),
  },
];

/**
 * Look up a style definition by its value key.
 * Falls back to the 'default' entry if not found.
 *
 * @param {string} key
 * @returns {OverlayStyleDef}
 */
export function getOverlayStyle(key) {
  return overlayStyles.find(s => s.value === key) || overlayStyles[0];
}

/**
 * Returns true if the given style key supports the opacity slider.
 *
 * @param {string} key
 * @returns {boolean}
 */
export function styleSupportsOpacity(key) {
  return getOverlayStyle(key).supportsOpacity;
}

/**
 * Returns true if the given style key should show the frost noise texture.
 *
 * @param {string} key
 * @returns {boolean}
 */
export function styleHasFrost(key) {
  return !!getOverlayStyle(key).hasFrost;
}

/**
 * Returns true when the given theme name maps to a dark colour scheme.
 * Mirrors the logic used by RecordingOverlay's `applyTheme()`.
 *
 * @param {string} theme
 * @returns {boolean}
 */
export function isDarkTheme(theme) {
  return theme?.startsWith('dark_') || theme === 'obsidian';
}

/**
 * Resolve the CSS property map for a given style key + theme name + opacity.
 *
 * @param {string} styleKey  e.g. 'frosted_glass'
 * @param {string} theme     e.g. 'dark_charcoal'
 * @param {number} [opacity=0.85] 0–1 value for glass styles
 * @returns {OverlayCSSProps}
 */
export function resolveOverlayCSS(styleKey, theme, opacity = 0.85) {
  const def = getOverlayStyle(styleKey);
  const fn = isDarkTheme(theme) ? def.dark : def.light;
  return fn(opacity);
}
