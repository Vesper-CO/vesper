import { appWindow, invoke, listen } from './api.js';
import { S, loadAll } from './state.js';
import { $, toast, errMsg } from './util.js';
import { buildNav, go, registerViews, showProgress, appendLog, syncPlayUI } from './shell.js';
import * as home from './views/home.js';
import * as instances from './views/instances.js';
import * as mods from './views/mods.js';
import * as accounts from './views/accounts.js';
import * as settings from './views/settings.js';

registerViews({ home, instances, mods, accounts, settings });

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
  if (S.settings.auto_update) {
    try {
      const v = await invoke('check_update');
      if (v) { toast(`Updating Vesper to ${v}...`); await invoke('install_update'); }
    } catch { /* offline or updates not configured: ignore */ }
  }
}
boot();
