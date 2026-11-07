export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => [...r.querySelectorAll(s)];
export const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
export const errMsg = (e) => (typeof e === 'string' ? e : e?.message || JSON.stringify(e));
export const fmtNum = (n) => (n >= 1e6 ? (n / 1e6).toFixed(1) + 'M' : n >= 1e3 ? (n / 1e3).toFixed(1) + 'k' : String(n ?? 0));
export const ramLabel = (mb) => (mb % 1024 ? (mb / 1024).toFixed(1) : mb / 1024) + ' GB';
/** Player head from the account's real skin (face + hat layer), pixel-perfect. Falls back to mc-heads. */
export const headHtml = (a, size = 34) => a?.skin
  ? `<span class="head" style="--s:${size}px;--skin:url('${esc(a.skin)}')"><i></i><i></i></span>`
  : `<img class="avatar" style="width:${size}px;height:${size}px" alt="" src="https://mc-heads.net/avatar/${esc(a?.uuid || '')}/${size * 2}">`;
export const LOADER_NAMES = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge', rust: 'Rust' };

export function ago(ts) {
  if (!ts) return 'Never played';
  const d = Math.floor((Date.now() / 1000 - ts) / 86400);
  return d <= 0 ? 'Played today' : d === 1 ? 'Played yesterday' : `Played ${d} days ago`;
}

const svg = (p) => `<svg viewBox="0 0 24 24">${p}</svg>`;
export const ICON = {
  home: svg('<path d="M3 11l9-8 9 8"/><path d="M5 10v10h5v-6h4v6h5V10"/>'),
  instances: svg('<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>'),
  mods: svg('<path d="M21 8l-9-5-9 5v8l9 5 9-5z"/><path d="M3 8l9 5 9-5M12 13v8"/>'),
  accounts: svg('<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 4-6 8-6s8 2 8 6"/>'),
  settings: svg('<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>'),
  folder: svg('<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>'),
  edit: svg('<path d="M4 20h4L19 9l-4-4L4 16z"/>'),
  trash: svg('<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>'),
  console: svg('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 10l3 2-3 2M12 15h5"/>'),
  close: svg('<path d="M6 6l12 12M18 6L6 18"/>'),
};

export function toast(msg, kind = 'info') {
  const el = document.createElement('div');
  el.className = 'toast ' + kind;
  el.textContent = msg;
  $('#toasts').append(el);
  setTimeout(() => el.classList.add('out'), 4200);
  setTimeout(() => el.remove(), 4600);
}

export function modal(html, { wide = false } = {}) {
  const root = $('#modal-root');
  root.innerHTML = `<div class="scrim"><div class="modal ${wide ? 'wide' : ''}" role="dialog">${html}</div></div>`;
  const close = () => { root.innerHTML = ''; document.removeEventListener('keydown', onKey); };
  const onKey = (e) => e.key === 'Escape' && close();
  document.addEventListener('keydown', onKey);
  $('.scrim', root).addEventListener('mousedown', (e) => e.target.classList.contains('scrim') && close());
  return { el: $('.modal', root), close };
}

export const skeleton = (n, cls = 'card') => Array.from({ length: n }, () => `<div class="${cls} skel"></div>`).join('');

/** Small, safe Markdown renderer for Modrinth descriptions: HTML is stripped, then everything is escaped. */
export function md(src = '') {
  let s = String(src)
    .replace(/<img[^>]*?src=["']([^"']+)["'][^>]*>/gi, '\n![]($1)\n')
    .replace(/<a[^>]*?href=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi, '[$2]($1)')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '');
  s = esc(s);
  const inline = (t) => t
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/!\[([^\]]*)\]\((https:[^)\s]+)[^)]*\)/g, '<img loading="lazy" alt="$1" src="$2">')
    .replace(/\[([^\]]+)\]\((https:[^)\s]+)[^)]*\)/g, '<a href="#" data-ext="$2">$1</a>')
    .replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>')
    .replace(/(^|[\s(])\*([^*\n]+)\*/g, '$1<i>$2</i>');
  const out = [];
  let list = false, code = false;
  for (const line of s.split('\n')) {
    if (/^\s*```/.test(line)) { out.push(code ? '</pre>' : (list ? '</ul><pre>' : '<pre>')); list = false; code = !code; continue; }
    if (code) { out.push(line + '\n'); continue; }
    const li = line.match(/^\s*[-*+]\s+(.*)/);
    if (li) { if (!list) out.push('<ul>'); list = true; out.push(`<li>${inline(li[1])}</li>`); continue; }
    if (list) { out.push('</ul>'); list = false; }
    const h = line.match(/^(#{1,6})\s+(.*)/);
    if (h) out.push(`<h${h[1].length < 3 ? 3 : 4}>${inline(h[2])}</h${h[1].length < 3 ? 3 : 4}>`);
    else if (line.trim()) out.push(`<p>${inline(line)}</p>`);
  }
  if (list) out.push('</ul>');
  if (code) out.push('</pre>');
  return out.join('');
}

/** In-app confirmation dialog (window.confirm is unreliable in webviews). */
export function ask(message, okLabel = 'Confirm') {
  return new Promise((resolve) => {
    const m = modal(`<h2>Are you sure?</h2><p>${esc(message)}</p><div class="foot"><button class="btn" id="no">Cancel</button><button class="btn primary" id="yes">${esc(okLabel)}</button></div>`);
    m.el.querySelector('#no').onclick = () => { m.close(); resolve(false); };
    m.el.querySelector('#yes').onclick = () => { m.close(); resolve(true); };
  });
}
