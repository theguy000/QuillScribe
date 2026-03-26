/** Canonical theme definitions — single source of truth. */
export const themes = [
  { value: 'white', label: 'White' },
  { value: 'warm_gray', label: 'Warm Gray' },
  { value: 'soft_beige', label: 'Soft Beige' },
  { value: 'blue_gray', label: 'Blue Gray' },
  { value: 'warm_taupe', label: 'Warm Taupe' },
  { value: 'soft_sage', label: 'Soft Sage' },
  { value: 'dark_charcoal', label: 'Dark Charcoal' },
  { value: 'dark_blue', label: 'Dark Blue' },
  { value: 'dark_purple', label: 'Dark Purple' },
  { value: 'dark_forest', label: 'Dark Forest' },
  { value: 'dark_burgundy', label: 'Dark Burgundy' },
  { value: 'obsidian', label: 'Obsidian' },
];

/** Flat list of CSS class names for theme removal. */
export const allThemeClasses = themes.map(t => t.value);
