import { startBackground } from './background.js';

const T = window.__TAURI__;
const invoke = (c, a) => T.core.invoke(c, a);
const win = T.window.getCurrentWindow();
const $ = (s) => document.querySelector(s);
const panel = $('#panel');
try { startBackground($('#bg')); } catch (e) { console.warn('background disabled', e); }

let busy = false, desktop = true, launch = true, exe = '';
$('#min').onclick = () => win.minimize();
$('#close').onclick = () => { if (!busy) win.close(); };
document.addEventListener('pointerdown', (e) => {
  const b = e.target.closest('.btn'); if (!b) return;
  const r = b.getBoundingClientRect(), d = Math.max(r.width, r.height) * 2, s = document.createElement('span');
  s.className = 'ripple'; s.style.cssText = `width:${d}px;height:${d}px;left:${e.clientX - r.left - d / 2}px;top:${e.clientY - r.top - d / 2}px`;
  b.append(s); setTimeout(() => s.remove(), 650);
});
const toggle = (el, get, set) => el.onclick = () => { set(!get()); el.classList.toggle('on', get()); el.setAttribute('aria-checked', get()); };
const CHECK = '<svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg>';
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

async function welcome() {
  const existing = await invoke('installed').catch(() => null);
  panel.innerHTML = `<img class="logo" src="assets/logo.svg" alt="">
    <h1>${existing ? 'Reinstall Vesper' : 'Install Vesper'}</h1>
    <p class="muted">A fast, small Minecraft launcher with Modrinth built in.<br>Installs just for you, with no admin rights needed.</p>
    <label class="opt"><button class="switch on" id="desk" role="switch" aria-checked="true" aria-label="Desktop shortcut"></button><span>Create a desktop shortcut</span></label>
    <button class="btn" id="go">${existing ? 'Reinstall' : 'Install'}</button>`;
  toggle($('#desk'), () => desktop, (v) => (desktop = v));
  $('#go').onclick = start;
}

const STEPS = [['prepare', 'Getting ready'], ['install', 'Installing Vesper'], ['shortcuts', 'Creating shortcuts']];
const TARGET = { prepare: 10, install: 20, shortcuts: 92, done: 100 };
let creep, pct = 0;
const setPct = (p) => { pct = p; const i = $('.bar i'); if (i) i.style.width = p + '%'; };

function installing() {
  busy = true;
  panel.innerHTML = `<img class="logo" src="assets/logo.svg" alt=""><h1>Installing</h1>
    <div class="steps">${STEPS.map(([k, t]) => `<div class="step" id="s-${k}"><span class="dot">${CHECK}</span>${t}</div>`).join('')}</div>
    <div class="bar"><i></i></div><small class="muted">Please keep this window open.</small>`;
  setPct(2);
}
function setStep(step) {
  const order = STEPS.map(([k]) => k);
  order.forEach((k, i) => {
    const el = $('#s-' + k); if (!el) return;
    const cur = order.indexOf(step);
    el.className = 'step ' + (step === 'done' || i < cur ? 'done' : i === cur ? 'active' : '');
  });
  setPct(TARGET[step] ?? pct);
  clearInterval(creep);
  if (step === 'install') creep = setInterval(() => setPct(pct + (86 - pct) * 0.06), 500); // slowly fills while the real installer runs
}

async function start() {
  installing();
  const off = await T.event.listen('stage', (e) => setStep(e.payload));
  try { exe = await invoke('install', { desktop }); setStep('done'); setTimeout(done, 600); }
  catch (e) { fail(e); }
  finally { off(); clearInterval(creep); }
}

function done() {
  busy = false; launch = true;
  panel.innerHTML = `<div class="tick"><svg viewBox="0 0 24 24"><path d="M5 12.5l4.5 4.5L19 7.5"/></svg></div>
    <h1>Vesper is installed</h1><p class="muted">You can find it in the Start menu${desktop ? ' and on your desktop' : ''}.</p>
    <label class="opt"><button class="switch on" id="run" role="switch" aria-checked="true" aria-label="Launch Vesper"></button><span>Launch Vesper now</span></label>
    <button class="btn" id="fin">Finish</button>`;
  toggle($('#run'), () => launch, (v) => (launch = v));
  $('#fin').onclick = () => invoke('finish', { launch, exe });
}

function fail(e) {
  busy = false;
  panel.innerHTML = `<h1>Something went wrong</h1><p class="err selectable">${esc(typeof e === 'string' ? e : e?.message || e)}</p>
    <div style="display:flex;gap:10px"><button class="btn ghost" id="x">Close</button><button class="btn" id="retry" style="font-size:15px;padding:10px 28px">Try again</button></div>`;
  $('#x').onclick = () => win.close(); $('#retry').onclick = welcome;
}

welcome();
