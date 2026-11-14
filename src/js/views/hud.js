// "In-game HUD": a live editor for the overlay that the Vesper HUD mod draws in the game
// (FPS, CPS, keystrokes...). Drag elements around the preview, and change every colour, the
// background (down to fully transparent), border, shape and text. Saved automatically.
import { invoke } from '../api.js';
import { $, esc, toast, errMsg } from '../util.js';
import { ELEMENTS, PRESETS, LOOK, defaultHud, mergeHud, hexA, styleOf } from '../hud-schema.js';

const GUI_W = 480; // the preview is drawn as if the game's GUI were 480 units wide (GUI scale 4 at 1080p)
let H, sel = 'fps', bgMode = 'day', saveTimer, root, resizeWatch;

const meta = (id) => ELEMENTS.find((e) => e.id === id);
const pct = (v) => Math.round(v * 100) + '%';

export async function render(el) {
  root = el;
  H = mergeHud(await invoke('get_hud').catch(() => null));
  if (!meta(sel)) sel = 'fps';
  el.innerHTML = `<div class="page">
    <div class="page-head"><div><h1>In-game HUD</h1><p class="muted">Drag the elements, then style each one. Changes are saved as you go and show up in a running game within a couple of seconds. You can also change everything in game: open the pause menu (Esc) and click the Vesper button next to the small icon buttons.</p></div>
      <div class="actions"><small class="muted" id="hud-status"></small><button class="btn" id="hud-reset">Reset all</button></div></div>
    <div class="hud-layout">
      <div class="hud-left">
        <div class="hud-stage ${bgMode}" id="hud-stage"></div>
        <div class="row hud-bgs"><small class="muted">Preview on</small>${['day', 'night', 'cave'].map((m) => `<button class="chip ${m === bgMode ? 'accent' : ''}" data-bg="${m}">${m[0].toUpperCase() + m.slice(1)}</button>`).join('')}</div>
        <div class="card" style="margin-top:14px"><h3>Style presets</h3><p class="muted" style="margin:0">Applies colours and shape to every element. Positions and on/off stay as they are.</p>
          <div class="chips" id="presets">${Object.keys(PRESETS).map((n) => `<button class="btn sm" data-preset="${esc(n)}">${esc(n)}</button>`).join('')}</div></div>
      </div>
      <div class="hud-panels" id="panel"></div>
    </div></div>`;
  drawStage(); drawPanel();
  resizeWatch?.disconnect(); // one watcher at a time, and only while this page exists
  resizeWatch = new ResizeObserver(() => { if (!$('#hud-stage')) { resizeWatch.disconnect(); return; } drawStage(); });
  resizeWatch.observe($('#hud-stage'));
  $('#hud-reset').onclick = () => { H = defaultHud(); sel = 'fps'; drawStage(); drawPanel(); saveSoon(); };
  el.querySelector('.hud-bgs').onclick = (e) => {
    const b = e.target.closest('[data-bg]'); if (!b) return;
    bgMode = b.dataset.bg; $('#hud-stage').className = `hud-stage ${bgMode}`;
    el.querySelectorAll('[data-bg]').forEach((c) => c.classList.toggle('accent', c === b));
  };
  $('#presets').onclick = (e) => {
    const n = e.target.closest('[data-preset]')?.dataset.preset; if (!n) return;
    for (const id in H.elements) Object.assign(H.elements[id], PRESETS[n]);
    drawStage(); drawPanel(); saveSoon();
  };
  setupDrag();
}

// ----- saving -----
function saveSoon() {
  const s = $('#hud-status'); if (s) s.textContent = 'Saving...';
  clearTimeout(saveTimer);
  saveTimer = setTimeout(async () => {
    try { await invoke('save_hud', { hud: H }); const s2 = $('#hud-status'); if (s2) s2.textContent = 'Saved'; } catch (e) { toast(errMsg(e), 'error'); }
  }, 300);
}

// ----- preview -----
function boxCss(c, px) {
  const b = c.borderWidth ? `${px(c.borderWidth)} solid ${hexA(c.border, c.borderOpacity)}` : 'none';
  return `background:${hexA(c.bg, c.bgOpacity)};border:${b};border-radius:${px(c.radius)};padding:${px(c.padding)} ${px(c.padding + 1)};`;
}
function textHtml(c, sample) {
  const [pre, post] = c.template.includes('{v}') ? c.template.split('{v}') : [c.template, null];
  const cls = c.rainbow ? 'rb' : '', lab = `style="${c.rainbow ? '' : `color:${c.labelColor}`}"`, val = `style="${c.rainbow ? '' : `color:${c.valueColor}`}"`;
  return `<span class="${cls}" ${lab}>${esc(pre)}</span>${post === null ? '' : `<span class="${cls}" ${val}>${esc(sample)}</span><span class="${cls}" ${lab}>${esc(post)}</span>`}`;
}
function innerHtml(id, c, px) {
  const m = meta(id), v = c.valueColor;
  if (id === 'keystrokes') {
    const key = (label, span, down) => `<b class="key" style="grid-column:span ${span};${boxCss(c, px)}${down ? `background:${hexA(v, 0.38)};` : ''}color:${v};height:${px(18)}">${label}</b>`;
    return `<div class="ks" style="display:grid;grid-template-columns:repeat(6,${px(9)});gap:${px(2)}"><span style="grid-column:span 2"></span>${key('W', 2, true)}<span style="grid-column:span 2"></span>${key('A', 2)}${key('S', 2)}${key('D', 2)}${key('LMB', 3)}${key('RMB', 3, true)}${key('SPACE', 6)}</div>`;
  }
  if (id === 'armor') return [['#7ee8f0', '363'], ['#7ee8f0', '528'], ['#7ee8f0', '495'], ['#7ee8f0', '429']].map(([col, d]) => `<div class="arm"><i style="background:${col};width:${px(9)};height:${px(9)}"></i><span style="color:${v}">${d}</span></div>`).join('');
  if (id === 'potions') return [['Speed II', '0:42'], ['Regeneration', '1:10']].map(([n, t]) => `<div>${textHtml({ ...c, template: n + ' {v}' }, t)}</div>`).join('');
  return textHtml(c, m.sample);
}
function drawStage() {
  const st = $('#hud-stage'); if (!st) return;
  const k = st.clientWidth / GUI_W, px = (n) => (n * k).toFixed(2) + 'px';
  st.style.opacity = H.enabled ? '' : '.55';
  st.innerHTML = ELEMENTS.filter((m) => H.elements[m.id].enabled).map((m) => {
    const c = H.elements[m.id], s = c.scale * H.scale, sh = c.shadow ? `text-shadow:${px(1)} ${px(1)} 0 rgba(0,0,0,.6);` : '';
    const wrap = m.special && m.id === 'keystrokes' ? '' : boxCss(c, px);
    return `<div class="hud-el ${m.id === sel ? 'sel' : ''}" data-id="${m.id}" style="font-size:${px(8 * s)};${sh}${wrap}">${innerHtml(m.id, c, (n) => px(n * s))}</div>`;
  }).join('');
  place();
}
function place() {
  const st = $('#hud-stage'); if (!st) return;
  for (const e of st.querySelectorAll('.hud-el')) {
    const c = H.elements[e.dataset.id];
    e.style.left = c.x * Math.max(0, st.clientWidth - e.offsetWidth) + 'px';
    e.style.top = c.y * Math.max(0, st.clientHeight - e.offsetHeight) + 'px';
  }
}
function setupDrag() {
  const st = $('#hud-stage'); let drag = null;
  st.addEventListener('pointerdown', (e) => {
    const el = e.target.closest('.hud-el'); if (!el) return;
    el.setPointerCapture(e.pointerId);
    drag = { el, id: el.dataset.id, dx: e.clientX - el.offsetLeft, dy: e.clientY - el.offsetTop };
    if (sel !== drag.id) { sel = drag.id; st.querySelectorAll('.hud-el').forEach((n) => n.classList.toggle('sel', n === el)); drawPanel(); }
    el.classList.add('drag');
  });
  st.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const { el } = drag, c = H.elements[drag.id];
    const rx = Math.max(1, st.clientWidth - el.offsetWidth), ry = Math.max(1, st.clientHeight - el.offsetHeight);
    let x = Math.min(1, Math.max(0, (e.clientX - drag.dx) / rx)), y = Math.min(1, Math.max(0, (e.clientY - drag.dy) / ry));
    if (Math.abs(x - 0.5) < 0.015) x = 0.5; if (Math.abs(y - 0.5) < 0.015) y = 0.5; // snap to the centre lines
    c.x = +x.toFixed(4); c.y = +y.toFixed(4);
    el.style.left = c.x * rx + 'px'; el.style.top = c.y * ry + 'px';
  });
  const end = () => { if (!drag) return; drag.el.classList.remove('drag'); drag = null; saveSoon(); };
  st.addEventListener('pointerup', end); st.addEventListener('pointercancel', end);
}

// ----- side panel -----
const sw = (k, on, label) => `<label class="row" style="gap:10px;flex-wrap:nowrap"><button class="switch ${on ? 'on' : ''}" data-sw="${k}" role="switch" aria-checked="${on}"></button><span>${label}</span></label>`;
const rng = (k, label, min, max, step, val, fmt) => `<label class="field"><span>${label}: <b data-o="${k}">${fmt(val)}</b></span><input type="range" data-k="${k}" data-fmt="${fmt === pct ? 'pct' : 'n'}" min="${min}" max="${max}" step="${step}" value="${val}"></label>`;
const col = (k, label, val) => `<label class="field"><span>${label}</span><input type="color" data-k="${k}" value="${val}"></label>`;

function drawPanel() {
  const p = $('#panel'); if (!p) return;
  const m = meta(sel), c = H.elements[sel], n = (v) => String(+v.toFixed(2));
  const item = (e) => `<div class="hud-item ${e.id === sel ? 'sel' : ''}" data-sel="${e.id}"><span>${esc(e.name)}</span><button class="switch sm ${H.elements[e.id].enabled ? 'on' : ''}" data-en="${e.id}" role="switch" aria-checked="${H.elements[e.id].enabled}" aria-label="Show ${esc(e.name)}"></button></div>`;
  // Each group of settings is its own small card, so no single card is tall.
  p.innerHTML = `<div class="card hud-card"><h3>Display</h3>${sw('master', H.enabled, 'Show the HUD in game')}${rng('gscale', 'Overall size', 0.5, 2, 0.05, H.scale, n)}</div>
    <div class="card hud-card"><h3>Elements</h3><div class="hud-list">${ELEMENTS.filter((e) => !e.addon).map(item).join('')}</div></div>
    <div class="card hud-card"><h3>Fabric add-on</h3><p class="muted" style="margin:0">These need data from inside the game, so they only show with the optional add-on (Fabric, Minecraft 1.21.4).</p>
      <div class="hud-list">${ELEMENTS.filter((e) => e.addon).map(item).join('')}</div></div>
    <div class="card hud-card"><h3>${esc(m.name)}: text</h3>
      ${m.addon ? '<p class="muted" style="margin:0">Needs the Fabric add-on to appear in game.</p>' : ''}
      ${m.special ? '' : `<label class="field"><span>Text (<code>{v}</code> is the value)</span><input type="text" data-k="template" maxlength="60" value="${esc(c.template)}"></label>`}
      <div class="row"><div class="grow">${col('labelColor', m.special && m.id !== 'potions' ? 'Key / icon colour' : 'Label colour', c.labelColor)}</div><div class="grow">${col('valueColor', 'Value colour', c.valueColor)}</div></div>
      <div class="row" style="gap:18px">${sw('rainbow', c.rainbow, 'Rainbow text')}${sw('shadow', c.shadow, 'Text shadow')}</div>
      ${rng('scale', 'Size', 0.5, 3, 0.05, c.scale, n)}</div>
    <div class="card hud-card"><h3>${esc(m.name)}: background</h3>${col('bg', 'Colour', c.bg)}
      ${rng('bgOpacity', 'Opacity', 0, 1, 0.01, c.bgOpacity, pct)}
      <div class="row"><button class="btn sm" data-q="clear">Fully transparent</button><button class="btn sm" data-q="solid">Solid</button></div></div>
    <div class="card hud-card"><h3>${esc(m.name)}: border and shape</h3>${col('border', 'Border colour', c.border)}
      ${rng('borderOpacity', 'Border opacity', 0, 1, 0.01, c.borderOpacity, pct)}${rng('borderWidth', 'Border width', 0, 4, 0.5, c.borderWidth, n)}
      ${rng('radius', 'Corner roundness', 0, 12, 1, c.radius, n)}${rng('padding', 'Padding', 0, 12, 1, c.padding, n)}</div>
    <div class="card hud-card"><h3>Copy and reset</h3><div class="row"><button class="btn sm" data-q="all">Use this style for all</button><button class="btn sm" data-q="reset">Reset ${esc(m.name)}</button></div>
      <p class="muted" style="margin:0">Works in every Minecraft 1.13+ instance on any loader: turn on <b>HUD</b> on an instance card.</p></div>`;
}

function changed() { drawStage(); saveSoon(); }
document.addEventListener('input', (e) => {
  const t = e.target; if (!t.closest?.('.hud-layout') || !t.dataset.k) return;
  const v = t.type === 'range' ? +t.value : t.value;
  if (t.dataset.k === 'gscale') H.scale = v; else H.elements[sel][t.dataset.k] = v;
  const o = t.closest('.field')?.querySelector('[data-o]'); if (o) o.textContent = t.dataset.fmt === 'pct' ? pct(v) : String(+v.toFixed(2));
  changed();
});
document.addEventListener('click', (e) => {
  if (!e.target.closest?.('.hud-layout')) return;
  const c = H.elements[sel];
  const s = e.target.closest('[data-sw]');
  if (s) { const k = s.dataset.sw; if (k === 'master') H.enabled = !H.enabled; else c[k] = !c[k]; drawPanel(); return changed(); }
  const en = e.target.closest('[data-en]');
  if (en) { const t = H.elements[en.dataset.en]; t.enabled = !t.enabled; sel = en.dataset.en; drawPanel(); return changed(); }
  const it = e.target.closest('[data-sel]');
  if (it) { sel = it.dataset.sel; drawPanel(); return drawStage(); }
  const q = e.target.closest('[data-q]')?.dataset.q;
  if (q === 'clear') { c.bgOpacity = 0; c.borderOpacity = 0; drawPanel(); changed(); }
  if (q === 'solid') { c.bgOpacity = 1; drawPanel(); changed(); }
  if (q === 'all') { for (const id in H.elements) Object.assign(H.elements[id], styleOf(c)); changed(); toast('Style copied to every element', 'ok'); }
  if (q === 'reset') { const d = defaultHud().elements[sel]; Object.assign(c, d, { enabled: c.enabled }); drawPanel(); changed(); }
});
