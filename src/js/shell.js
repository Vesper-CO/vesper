// App chrome: navigation, router, account chip, progress dock, console drawer and play helper.
import { invoke } from './api.js';
import { S, activeAccount } from './state.js';
import { $, $$, esc, md, modal, ICON, toast, errMsg, headHtml } from './util.js';

const NAV = [['home', 'Home'], ['instances', 'Instances'], ['mods', 'Mod Library'], ['accounts', 'Accounts'], ['settings', 'Settings']];
let views = {};
export const registerViews = (v) => (views = v);

export function buildNav() {
  $('#nav').innerHTML = '<span class="nav-ind"></span>' + NAV.map(([id, label]) => `<button class="nav-item" data-nav="${id}">${ICON[id]}<span>${label}</span></button>`).join('');
  $('#nav').addEventListener('click', (e) => { const b = e.target.closest('[data-nav]'); if (b) go(b.dataset.nav); });
  $('#acct-chip').addEventListener('click', () => go('accounts'));
  renderChip();
}

export async function go(view) {
  S.view = view;
  $$('.nav-item').forEach((b) => b.classList.toggle('active', b.dataset.nav === view));
  const act = $('.nav-item.active'), ind = $('.nav-ind');
  if (act && ind) { ind.style.height = act.offsetHeight + 'px'; ind.style.transform = `translateY(${act.offsetTop}px)`; }
  const el = $('#view');
  el.scrollTop = 0;
  try { await views[view].render(el); } catch (e) { el.innerHTML = `<div class="page"><div class="empty">Something went wrong: ${esc(errMsg(e))}</div></div>`; }
}

export function renderChip() {
  const a = activeAccount();
  $('#acct-chip').innerHTML = a
    ? `${headHtml(a, 34)}<span class="meta"><b>${esc(a.name)}</b><small>Signed in</small></span>`
    : `<span class="avatar"></span><span class="meta"><b>Sign in</b><small>Microsoft account</small></span>`;
}

// ----- progress dock -----
let hideTimer;
export function showProgress({ label, done, total }) {
  const dock = $('#dock');
  let box = $('.progress', dock);
  if (!box) { dock.innerHTML = '<div class="progress"><div class="row" style="justify-content:space-between"><b></b><small class="muted"></small></div><div class="bar"><i></i></div></div>'; box = $('.progress', dock); }
  $('b', box).textContent = label;
  $('small', box).textContent = `${done} / ${total}`;
  $('i', box).style.width = `${total ? (done / total) * 100 : 0}%`;
  clearTimeout(hideTimer);
  if (done >= total) hideTimer = setTimeout(() => ($('#dock').innerHTML = ''), 1400);
}

// ----- console drawer -----
let openId = null;
const lineHtml = ({ line, stream }) => {
  const lvl = (line.match(/\/(INFO|WARN|ERROR|FATAL)\]/) || [])[1] || '';
  return `<div class="${stream === 'err' ? 'err' : ''} ${lvl}">${esc(line)}</div>`;
};
export function openConsole(id) {
  openId = id;
  const inst = S.instances.find((i) => i.id === id);
  $('#console-root').innerHTML = `<div class="console"><header><b class="grow">${esc(inst?.name || 'Game log')}</b>
    <button class="btn sm danger" data-c="kill">Stop game</button><button class="btn sm" data-c="copy">Copy</button><button class="btn sm" data-c="clear">Clear</button>
    <button class="icon-btn" data-c="close" aria-label="Close">${ICON.close}</button></header><div class="log selectable">${(S.logs[id] || []).map(lineHtml).join('')}</div></div>`;
  const log = $('.log'); log.scrollTop = log.scrollHeight;
  $('.console').addEventListener('click', async (e) => {
    const c = e.target.closest('[data-c]')?.dataset.c;
    if (c === 'close') closeConsole();
    if (c === 'kill') invoke('kill_game', { id });
    if (c === 'clear') { S.logs[id] = []; $('.log').innerHTML = ''; }
    if (c === 'copy') { await navigator.clipboard.writeText((S.logs[id] || []).map((l) => l.line).join('\n')); toast('Log copied', 'ok'); }
  });
}
export const closeConsole = () => { openId = null; $('#console-root').innerHTML = ''; };
export function appendLog(id, line, stream = 'out') {
  const list = (S.logs[id] ||= []);
  list.push({ line, stream });
  if (list.length > 5000) list.splice(0, 1000);
  if (openId === id) {
    const log = $('.log');
    if (!log) return;
    const stick = log.scrollHeight - log.scrollTop - log.clientHeight < 80;
    log.insertAdjacentHTML('beforeend', lineHtml({ line, stream }));
    if (stick) log.scrollTop = log.scrollHeight;
  }
}

// ----- play -----
export function syncPlayUI() {
  $$('[data-play]').forEach((b) => {
    const st = S.running[b.dataset.play];
    b.disabled = !!st && st !== 'Running';
    b.dataset.state = st ? 'busy' : '';
    if (b.classList.contains('play')) b.lastChild.textContent = st === 'Running' ? 'Running' : st ? 'Starting' : 'Play';
  });
  const st = S.running[S.homeInstance];
  const el = $('#stage'); if (el) el.textContent = st && st !== 'Running' ? st : '';
}

export async function play(id) {
  if (!S.accounts.length) { toast('Sign in with a Microsoft account first.', 'error'); return go('accounts'); }
  if (S.running[id]) return openConsole(id);
  S.logs[id] = [];
  S.running[id] = 'Starting';
  syncPlayUI();
  openConsole(id);
  try {
    await invoke('play', { id });
    S.running[id] = 'Running';
  } catch (e) {
    delete S.running[id];
    appendLog(id, 'Launch failed: ' + errMsg(e), 'err');
    toast(errMsg(e), 'error');
  }
  syncPlayUI();
}

// ----- update prompt -----
export function promptUpdate(info) {
  const m = modal(`<h2>Vesper ${esc(info.version)} is available</h2>
    <p class="muted">You are on version ${esc(info.current)}.</p>
    ${info.notes ? `<div class="desc selectable" style="max-height:240px;overflow:auto;margin-top:12px">${md(info.notes)}</div>` : ''}
    <div class="foot"><button class="btn" id="later">Later</button><button class="btn primary" id="now">Update now</button></div>`);
  m.el.querySelector('#later').onclick = m.close;
  const now = m.el.querySelector('#now');
  now.onclick = async () => {
    now.disabled = true; now.textContent = 'Downloading...';
    m.el.querySelector('#later').disabled = true;
    try { await invoke('install_update'); } // the app restarts when this succeeds
    catch (e) { toast(errMsg(e), 'error'); now.disabled = false; now.textContent = 'Update now'; m.el.querySelector('#later').disabled = false; }
  };
}
