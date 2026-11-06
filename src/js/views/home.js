import { invoke } from '../api.js';
import { S, activeAccount } from '../state.js';
import { esc, skeleton, LOADER_NAMES, ramLabel, ICON } from '../util.js';
import { go, play, syncPlayUI } from '../shell.js';

export async function render(el) {
  const acct = activeAccount();
  if (!S.instances.some((i) => i.id === S.homeInstance)) S.homeInstance = S.instances[0]?.id ?? null;
  const recent = S.instances.filter((i) => i.last_played).slice(0, 4);

  el.innerHTML = `<div class="page">
    <section class="hero">
      <div>
        <h1>${acct ? `Welcome back, ${esc(acct.name)}` : 'Welcome to Vesper'}</h1>
        <p class="muted" style="margin-top:6px">${acct ? 'Pick an instance and jump in.' : 'Sign in with Microsoft to start playing.'}</p>
      </div>
      <div class="play-box">
        ${S.instances.length
          ? `<select id="home-inst" aria-label="Instance">${S.instances.map((i) => `<option value="${i.id}" ${i.id === S.homeInstance ? 'selected' : ''}>${esc(i.name)}</option>`).join('')}</select>
             <button class="btn primary play" id="play" data-play="${S.homeInstance}"><svg viewBox="0 0 24 24"><path d="M7 4l13 8-13 8z"/></svg><span>Play</span></button>`
          : `<button class="btn primary play" id="new"><span>Create your first instance</span></button>`}
        <div class="stage" id="stage"></div>
      </div>
    </section>
    ${recent.length ? `<h2>Recent instances</h2><div class="grid">${recent.map((i) => `
      <button class="card" data-open="${i.id}" style="text-align:left">
        <h3>${esc(i.name)}</h3>
        <div class="chips"><span class="chip accent">${LOADER_NAMES[i.loader]}</span><span class="chip">${esc(i.mc_version)}</span><span class="chip">${ramLabel(i.ram_mb)}</span></div>
      </button>`).join('')}</div>` : ''}
    <h2>Minecraft news</h2>
    <div class="news" id="news">${skeleton(3, 'news-card')}</div>
  </div>`;

  el.querySelector('#home-inst')?.addEventListener('change', (e) => { S.homeInstance = e.target.value; el.querySelector('#play').dataset.play = S.homeInstance; syncPlayUI(); });
  el.querySelector('#play')?.addEventListener('click', () => play(S.homeInstance));
  el.querySelector('#new')?.addEventListener('click', () => go('instances'));
  el.querySelectorAll('[data-open]').forEach((b) => b.addEventListener('click', () => { S.homeInstance = b.dataset.open; render(el); }));
  syncPlayUI();

  const box = el.querySelector('#news');
  invoke('get_news').then((items) => {
    box.innerHTML = items.length ? items.map((n) => `<button class="news-card" data-ext="https://www.minecraft.net/en-us/updates">
        ${n.image ? `<img loading="lazy" alt="" src="${esc(n.image)}">` : '<img alt="">'}
        <div><b>${esc(n.title)}</b><small class="muted">Java Edition ${esc(n.version)}</small></div></button>`).join('')
      : '<p class="muted">No news right now.</p>';
    box.querySelectorAll('[data-ext]').forEach((b) => b.addEventListener('click', () => invoke('open_url', { url: b.dataset.ext })));
  }).catch(() => (box.innerHTML = '<p class="muted">News could not be loaded. Check your connection.</p>'));
}
