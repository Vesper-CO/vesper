// Settings for the in-game HUD, shared by the editor page and (as saved JSON) by the HUD mod.
// Positions are fractions: x = 0 puts the element against the left edge of the screen, 1 against the
// right edge; y likewise for top and bottom. Sizes are in Minecraft GUI units, so they scale with the
// player's GUI scale. The mod reads the same names, so keep the two in sync (hud-mod/.../HudConfig.java).
export const ELEMENTS = [
  { id: 'fps', name: 'FPS', tpl: 'FPS: {v}', sample: '144', x: 0.01, y: 0.02, on: true },
  { id: 'cps', name: 'CPS', tpl: 'CPS: {v}', sample: '8 | 5', x: 0.01, y: 0.1, on: true },
  { id: 'ping', name: 'Ping', tpl: 'Ping: {v}', sample: '32 ms', x: 0.01, y: 0.18, on: true },
  { id: 'coords', name: 'Coordinates', tpl: 'XYZ: {v}', sample: '120 / 64 / -340', x: 0.01, y: 0.26, on: true },
  { id: 'direction', name: 'Direction', tpl: 'Facing: {v}', sample: 'North', x: 0.01, y: 0.34, on: false },
  { id: 'memory', name: 'Memory', tpl: 'RAM: {v}', sample: '1.4 / 4.0 GB', x: 0.01, y: 0.42, on: false },
  { id: 'clock', name: 'Clock', tpl: '{v}', sample: '21:45', x: 0.5, y: 0.01, on: false },
  { id: 'keystrokes', name: 'Keystrokes', tpl: '', sample: '', x: 0.01, y: 0.72, on: true, special: true },
  { id: 'armor', name: 'Armor status', tpl: '', sample: '', x: 0.96, y: 0.74, on: true, special: true },
  { id: 'potions', name: 'Potion effects', tpl: '', sample: '', x: 0.99, y: 0.02, on: false, special: true },
];

export const LOOK = { scale: 1, labelColor: '#c9bfff', valueColor: '#ffffff', rainbow: false, shadow: true, bg: '#0b0b17', bgOpacity: 0.55, border: '#8f7cff', borderOpacity: 0.35, borderWidth: 1, radius: 4, padding: 4 };
// What a preset changes: colours and shape only, never position or on/off.
export const PRESETS = {
  'Vesper Night': { ...LOOK, radius: 5 },
  'Vesper Day': { ...LOOK, labelColor: '#1f64d6', valueColor: '#14213d', shadow: false, bg: '#ffffff', bgOpacity: 0.62, border: '#1f64d6', borderOpacity: 0.35, radius: 5 },
  Glass: { ...LOOK, labelColor: '#e8eeff', bg: '#ffffff', bgOpacity: 0.12, border: '#ffffff', borderOpacity: 0.4, radius: 7 },
  Transparent: { ...LOOK, labelColor: '#ffffff', bgOpacity: 0, borderOpacity: 0, radius: 0, padding: 2 },
  Neon: { ...LOOK, labelColor: '#3dfcff', valueColor: '#ff7af5', bg: '#05020f', bgOpacity: 0.65, border: '#ff3df2', borderOpacity: 0.9, radius: 3 },
};
const STYLE_KEYS = Object.keys(LOOK);

export function defaultHud() {
  const elements = {};
  for (const e of ELEMENTS) elements[e.id] = { enabled: e.on, x: e.x, y: e.y, template: e.tpl, ...LOOK };
  return { version: 1, enabled: true, scale: 1, elements };
}

const num = (v, lo, hi, d) => (Number.isFinite(+v) ? Math.min(hi, Math.max(lo, +v)) : d);
const hex = (v, d) => (/^#[0-9a-f]{6}$/i.test(v) ? v.toLowerCase() : d);

// Fill in anything missing or out of range, so old or hand-edited files never break the editor.
export function mergeHud(saved) {
  const d = defaultHud();
  if (!saved || typeof saved !== 'object') return d;
  d.enabled = saved.enabled !== false;
  d.scale = num(saved.scale, 0.5, 3, 1);
  for (const e of ELEMENTS) {
    const s = saved.elements?.[e.id] || {}, t = d.elements[e.id];
    t.enabled = typeof s.enabled === 'boolean' ? s.enabled : t.enabled;
    t.x = num(s.x, 0, 1, t.x); t.y = num(s.y, 0, 1, t.y);
    if (typeof s.template === 'string' && s.template.length <= 60) t.template = s.template;
    t.scale = num(s.scale, 0.5, 3, t.scale);
    for (const k of ['labelColor', 'valueColor', 'bg', 'border']) t[k] = hex(s[k], t[k]);
    for (const k of ['bgOpacity', 'borderOpacity']) t[k] = num(s[k], 0, 1, t[k]);
    t.borderWidth = num(s.borderWidth, 0, 4, t.borderWidth); t.radius = num(s.radius, 0, 12, t.radius); t.padding = num(s.padding, 0, 12, t.padding);
    t.rainbow = s.rainbow === true; t.shadow = s.shadow !== false;
  }
  return d;
}

export const styleOf = (el) => Object.fromEntries(STYLE_KEYS.map((k) => [k, el[k]]));
export const hexA = (h, a) => { const n = parseInt(h.slice(1), 16); return `rgba(${n >> 16},${(n >> 8) & 255},${n & 255},${a})`; };
