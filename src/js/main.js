import { appWindow, invoke, listen } from './api.js';
import { S, loadAll, refreshAccounts } from './state.js';
import { $, toast, errMsg } from './util.js';
import { buildNav, go, registerViews, showProgress, appendLog, syncPlayUI, promptUpdate, renderChip } from './shell.js';
import { startBackground } from './background.js';
import * as home from './views/home.js';
import * as instances from './views/instances.js';
import * as mods from './views/mods.js';
import * as accounts from './views/accounts.js';
import * as settings from './views/settings.js';

registerViews({ home, instances, mods, accounts, settings });
// The background is decoration: if it ever fails, the app must still work.
try { startBackground(document.getElementById('bg')); } catch (e) { console.warn('background disabled', e); }

// Cursor spotlight and gentle tilt on cards.
const HOVERABLE = '.card,.mod,.news-card';
document.addEventListener('pointermove', (e) => {
  const el = e.target.closest?.(HOVERABLE); if (!el) return;
  const r = el.getBoundingClientRect(), x = e.clientX - r.left, y = e.clientY - r.top;
  el.style.setProperty('--mx', x + 'px'); el.style.setProperty('--my', y + 'px');
  if (r.width < 520) { el.style.setProperty('--ry', ((x / r.width - 0.5) * 9).toFixed(2) + 'deg'); el.style.setProperty('--rx', ((0.5 - y / r.height) * 9).toFixed(2) + 'deg'); }
});
document.addEventListener('pointerout', (e) => {
  const el = e.target.closest?.(HOVERABLE);
  if (el && !el.contains(e.relatedTarget)) { el.style.removeProperty('--rx'); el.style.removeProperty('--ry'); }
});
// Button ripple.
document.addEventListener('pointerdown', (e) => {
  const b = e.target.closest('.btn'); if (!b || b.disabled) return;
  const r = b.getBoundingClientRect(), d = Math.max(r.width, r.height) * 2, s = document.createElement('span');
  s.className = 'ripple'; s.style.cssText = `width:${d}px;height:${d}px;left:${e.clientX - r.left - d / 2}px;top:${e.clientY - r.top - d / 2}px`;
  b.append(s); setTimeout(() => s.remove(), 650);
});

document.addEventListener('click', (e) => {
  const a = e.target.closest('a[data-ext]');
  if (a) { e.preventDefault(); invoke('open_url', { url: a.dataset.ext }).catch(() => {}); }
});
document.addEventListener('contextmenu', (e) => !e.target.closest('input,textarea,.log') && e.preventDefault());

$('.wc').addEventListener('click', (e) => {
  const w = e.target.closest('[data-win]')?.dataset.win;
  if (w === 'min') appWindow.minimize();
  if (w === 'max') appWindow.toggleMaximize();
  if (w === 'close') appWindow.close();
});

listen('progress', showProgress);
listen('game-log', ({ id, line, stream }) => appendLog(id, line, stream));
listen('launch-stage', ({ id, stage }) => { S.running[id] = stage; syncPlayUI(); });
listen('game-exit', ({ id, code }) => {
  delete S.running[id];
  appendLog(id, `Game closed (exit code ${code ?? 'killed'})`, code ? 'err' : 'out');
  syncPlayUI();
});

async function boot() {
  try { await loadAll(); } catch (e) { toast(errMsg(e), 'error'); }
  buildNav();
  go('home');
  // Refresh tokens and fetch the real skin, then redraw the account chip.
  invoke('refresh_profile').then(async () => { await refreshAccounts(); renderChip(); }).catch(() => {});
  if (S.settings.auto_update) {
    try {
      const info = await invoke('check_update');
      if (info) promptUpdate(info);
    } catch { /* offline or updates not configured: ignore */ }
  }
}
boot();
