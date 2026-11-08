import { invoke } from '../api.js';
import { S, refreshInstances } from '../state.js';
import { $, esc, md, modal, toast, errMsg, fmtNum, skeleton, LOADER_NAMES } from '../util.js';
import { go } from '../shell.js';

const TYPES = [['mod', 'Mods'], ['resourcepack', 'Resource packs'], ['shader', 'Shaders'], ['modpack', 'Modpacks'], ['installed', 'Installed']];
const SORTS = [['relevance', 'Most relevant'], ['downloads', 'Most downloaded'], ['follows', 'Most followed'], ['newest', 'Newest'], ['updated', 'Recently updated']];
const M = { type: 'mod', q: '', cat: '', sort: 'relevance', compat: true, offset: 0, hits: [], total: 0, cats: null, busy: false };
const LOADER_COMPAT = { fabric: ['fabric'], quilt: ['quilt', 'fabric'], forge: ['forge'], neoforge: ['neoforge'], vanilla: [] };
const RUST_TYPES = [['mod', 'Mods'], ['resourcepack', 'Resource packs'], ['shader', 'Shaders'], ['installed', 'Installed']];
const H = { q: '', native: false, offset: 0, hits: [], total: 0, busy: false };
const isRust = (i) => i.loader === 'rust';
const typesFor = (i) => (isRust(i) ? RUST_TYPES : TYPES);
const eligible = () => S.instances;
const target = () => eligible().find((i) => i.id === S.target) || eligible()[0];

export async function render(el) {
  await refreshInstances();
  if (!eligible().length) {
    el.innerHTML = `<div class="page"><div class="page-head"><h1>Mod Library</h1></div><div class="empty"><b>Create an instance first</b><span>Content is installed into an instance, so Vesper can check that it fits its version and loader.</span><button class="btn primary" id="go">Go to Instances</button></div></div>`;
    $('#go').onclick = () => go('instances'); return;
  }
  if (!eligible().some((i) => i.id === S.target)) S.target = eligible()[0].id;
  const inst = target();
  if (isRust(inst) && M.type === 'modpack') M.type = 'mod';
  el.innerHTML = `<div class="page">
    <div class="page-head"><div><h1>Mod Library</h1><p class="muted">${isRust(inst) ? 'Mods from the Horizon library, packs from Modrinth' : 'Powered by Modrinth'}</p></div>
      <label class="field" style="margin:0;min-width:260px"><span>Install into</span><select id="inst">${eligible().map((i) => `<option value="${i.id}" ${i.id === S.target ? 'selected' : ''}>${esc(i.name)} (${LOADER_NAMES[i.loader]} ${esc(i.mc_version)})</option>`).join('')}</select></label></div>
    <div class="tabs" id="tabs">${typesFor(inst).map(([k, v]) => `<button class="tab ${M.type === k ? 'active' : ''}" data-t="${k}">${v}</button>`).join('')}</div>
    <div id="body"></div></div>`;
  $('#inst').onchange = (e) => { S.target = e.target.value; render(el); };
  $('#tabs').onclick = (e) => { const t = e.target.closest('[data-t]')?.dataset.t; if (t) { M.type = t; M.offset = 0; M.hits = []; render(el); } };
  if (M.type === 'installed') return installed(inst);
  if (isRust(inst) && M.type === 'mod') return browseHorizon(inst);
  await browse(inst);
}

// ---------- Horizon's mod library (Rust instances) ----------

const NATIVE_NOTE = 'Native mods are Rust code compiled into the game itself. Vesper cannot build that yet, so they are listed but cannot be installed.';

async function browseHorizon(inst) {
  const body = $('#body');
  body.innerHTML = `<div class="filters">
      <input type="search" id="q" placeholder="Search the Horizon library" value="${esc(H.q)}" aria-label="Search">
      <label class="row" style="gap:8px;flex-wrap:nowrap"><button class="switch ${H.native ? 'on' : ''}" id="nat" role="switch" aria-checked="${H.native}"></button><span>Show native mods</span></label></div>
    <p class="muted" id="count"></p><p class="muted" id="hnote" ${H.native ? '' : 'hidden'}>${NATIVE_NOTE}</p>
    <div class="list" id="hits"></div><div class="row" style="justify-content:center;margin-top:16px"><button class="btn" id="more" hidden>Load more</button></div>`;
  let timer;
  $('#q').oninput = (e) => { clearTimeout(timer); timer = setTimeout(() => { H.q = e.target.value; hsearch(inst, true); }, 350); };
  $('#nat').onclick = (e) => { H.native = !H.native; e.target.classList.toggle('on', H.native); e.target.setAttribute('aria-checked', H.native); $('#hnote').hidden = !H.native; hsearch(inst, true); };
  $('#more').onclick = () => hsearch(inst, false);
  $('#hits').onclick = (e) => {
    const card = e.target.closest('[data-h]'); if (!card) return;
    if (e.target.closest('[data-i]')) return hInstall(inst, card.dataset.h, e.target.closest('[data-i]'));
    openHorizon(inst, card.dataset.h);
  };
  H.hits.length ? hpaint() : hsearch(inst, true);
}

async function hsearch(inst, reset) {
  if (H.busy) return;
  H.busy = true;
  if (reset) { H.offset = 0; H.hits = []; $('#hits').innerHTML = skeleton(6, 'mod'); }
  try {
    const r = await invoke('horizon_search', { instanceId: inst.id, query: H.q, showNative: H.native, offset: H.offset });
    H.hits = reset ? r.hits : H.hits.concat(r.hits); H.total = r.total; H.offset = H.hits.length;
    hpaint();
  } catch (e) { const box = $('#hits'); if (box) box.innerHTML = `<div class="empty">Could not load the Horizon library: ${esc(errMsg(e))}</div>`; }
  H.busy = false;
}

const hButton = (h) => {
  if (h.installed && !h.update) return '<button class="btn sm" disabled>Installed</button>';
  if (h.installed && h.update) return '<button class="btn primary sm" data-i="1">Update</button>';
  if (h.support === 'ok') return '<button class="btn primary sm" data-i="1">Install</button>';
  const why = { native: 'Needs a native build', code: 'Not supported', version: 'No 26.3 version', other: 'Not a mod' }[h.support] || 'Unavailable';
  return `<button class="btn sm" disabled>${why}</button>`;
};

function hpaint() {
  const box = $('#hits'); if (!box) return;
  $('#count').textContent = H.total ? `${fmtNum(H.total)} ${H.total === 1 ? 'mod' : 'mods'}` : '';
  box.innerHTML = H.hits.length ? H.hits.map((h) => `<div class="mod" data-h="${esc(h.id)}" tabindex="0">
      ${h.icon ? `<img class="icon" loading="lazy" alt="" src="${esc(h.icon)}">` : '<div class="icon"></div>'}
      <div style="min-width:0"><div class="t">${esc(h.name)} <small class="muted">by ${esc((h.authors || []).join(', ') || 'unknown')}</small></div><p>${esc(h.summary)}</p>
        <div class="chips" style="margin-top:6px"><span class="chip">${fmtNum(h.downloads)} downloads</span>${h.version ? `<span class="chip">${esc(h.version)}</span>` : ''}${h.kind === 'native-mod' ? '<span class="chip">Native</span>' : ''}${(h.categories || []).slice(0, 2).map((c) => `<span class="chip">${esc(c)}</span>`).join('')}</div></div>
      ${hButton(h)}</div>`).join('')
    : `<div class="empty">${H.native ? 'Nothing found. Try another search.' : 'Nothing found. Native mods are hidden: turn on "Show native mods" to see them.'}</div>`;
  $('#more').hidden = H.hits.length >= H.total;
}

async function hInstall(inst, id, btn) {
  btn.disabled = true; const old = btn.textContent; btn.textContent = 'Installing...';
  try {
    const name = await invoke('horizon_install', { instanceId: inst.id, id });
    toast(`Installed ${name}. It is switched on: start the game to use it.`, 'ok');
    const h = H.hits.find((x) => x.id === id); if (h) { h.installed = true; h.update = false; }
    btn.textContent = 'Installed';
  } catch (e) { toast(errMsg(e), 'error'); btn.disabled = false; btn.textContent = old; }
}

async function openHorizon(inst, id) {
  const m = modal('<div class="skel" style="height:220px"></div>', { wide: true });
  let p;
  try { p = await invoke('horizon_project', { instanceId: inst.id, id }); } catch (e) { m.close(); return toast(errMsg(e), 'error'); }
  m.el.innerHTML = `<div class="row" style="flex-wrap:nowrap;align-items:flex-start;gap:16px">
      ${p.icon ? `<img class="icon" style="width:72px;height:72px;border-radius:14px" alt="" src="${esc(p.icon)}">` : ''}
      <div class="grow"><h2 style="margin:0">${esc(p.name)}</h2><p class="muted">${esc(p.summary)}</p>
        <div class="chips" style="margin-top:6px"><span class="chip">${fmtNum(p.downloads)} downloads</span>${p.version ? `<span class="chip">${esc(p.version)}</span>` : ''}${p.license ? `<span class="chip">${esc(p.license)}</span>` : ''}</div></div>
      <button class="btn" id="pg">Open page</button><span id="ib">${hButton(p)}</span><button class="icon-btn" id="x" aria-label="Close">&#10005;</button></div>
    ${p.support === 'native' ? `<p class="muted" style="margin-top:12px">${NATIVE_NOTE}</p>` : ''}
    <div class="desc selectable" style="margin-top:16px">${md(p.description || p.summary)}</div>`;
  $('#x', m.el).onclick = m.close;
  $('#pg', m.el).onclick = () => invoke('open_url', { url: p.page });
  const ib = $('#ib [data-i]', m.el);
  if (ib) ib.onclick = () => hInstall(inst, id, ib);
}

async function browse(inst) {
  const body = $('#body');
  body.innerHTML = `<div class="filters">
      <input type="search" id="q" placeholder="Search Modrinth" value="${esc(M.q)}" aria-label="Search">
      <select id="cat" aria-label="Category"><option value="">All categories</option></select>
      <select id="sort" aria-label="Sort">${SORTS.map(([k, v]) => `<option value="${k}" ${M.sort === k ? 'selected' : ''}>${v}</option>`).join('')}</select>
      <label class="row" style="gap:8px;flex-wrap:nowrap"><button class="switch ${M.compat ? 'on' : ''}" id="compat" role="switch" aria-checked="${M.compat}"></button><span>Only compatible</span></label></div>
    <p class="muted" id="count"></p><div class="list" id="hits"></div><div class="row" style="justify-content:center;margin-top:16px"><button class="btn" id="more" hidden>Load more</button></div>`;
  try {
    M.cats ??= await invoke('modrinth_categories');
    const cats = M.cats.filter((c) => c.project_type === (M.type === 'modpack' ? 'modpack' : M.type) && c.header === 'categories');
    $('#cat').innerHTML += cats.map((c) => `<option value="${esc(c.name)}" ${M.cat === c.name ? 'selected' : ''}>${esc(c.name[0].toUpperCase() + c.name.slice(1))}</option>`).join('');
  } catch { /* filters optional */ }
  let timer;
  $('#q').oninput = (e) => { clearTimeout(timer); timer = setTimeout(() => { M.q = e.target.value; search(inst, true); }, 350); };
  $('#cat').onchange = (e) => { M.cat = e.target.value; search(inst, true); };
  $('#sort').onchange = (e) => { M.sort = e.target.value; search(inst, true); };
  $('#compat').onclick = (e) => { M.compat = !M.compat; e.target.classList.toggle('on', M.compat); e.target.setAttribute('aria-checked', M.compat); search(inst, true); };
  $('#more').onclick = () => search(inst, false);
  $('#hits').onclick = (e) => {
    const card = e.target.closest('[data-p]'); if (!card) return;
    if (e.target.closest('[data-i]')) return quickInstall(inst, card.dataset.p, e.target.closest('[data-i]'));
    openProject(card.dataset.p);
  };
  M.hits.length ? paint() : search(inst, true);
}

async function search(inst, reset) {
  if (M.busy) return;
  M.busy = true;
  if (reset) { M.offset = 0; M.hits = []; $('#hits').innerHTML = skeleton(6, 'mod'); }
  try {
    const r = await invoke('modrinth_search', {
      query: M.q, projectType: M.type, categories: M.cat ? [M.cat] : [], sort: M.sort, offset: M.offset,
      loader: M.compat && M.type === 'mod' ? (inst.loader === 'vanilla' ? '' : inst.loader) : '', version: M.compat ? inst.mc_version : '',
    });
    M.hits = reset ? r.hits : M.hits.concat(r.hits); M.total = r.total_hits; M.offset = M.hits.length;
    paint();
  } catch (e) { $('#hits').innerHTML = `<div class="empty">Could not reach Modrinth: ${esc(errMsg(e))}</div>`; }
  M.busy = false;
}

function paint() {
  const box = $('#hits'); if (!box) return;
  $('#count').textContent = M.total ? `${fmtNum(M.total)} results` : '';
  box.innerHTML = M.hits.length ? M.hits.map((h) => `<div class="mod" data-p="${esc(h.project_id)}" tabindex="0">
      ${h.icon_url ? `<img class="icon" loading="lazy" alt="" src="${esc(h.icon_url)}">` : '<div class="icon"></div>'}
      <div style="min-width:0"><div class="t">${esc(h.title)} <small class="muted">by ${esc(h.author)}</small></div><p>${esc(h.description)}</p>
        <div class="chips" style="margin-top:6px"><span class="chip">${fmtNum(h.downloads)} downloads</span>${(h.display_categories || []).slice(0, 3).map((c) => `<span class="chip">${esc(c)}</span>`).join('')}</div></div>
      <button class="btn primary sm" data-i="1">${M.type === 'modpack' ? 'Install as instance' : 'Install'}</button></div>`).join('')
    : '<div class="empty">Nothing found. Try another search, or turn off "Only compatible".</div>';
  $('#more').hidden = M.hits.length >= M.total;
}

async function quickInstall(inst, pid, btn, versionId) {
  btn.disabled = true; const old = btn.textContent; btn.textContent = 'Installing...';
  try {
    if (M.type === 'modpack') {
      const i = await invoke('modrinth_install_modpack', { projectId: pid, versionId: versionId ?? null });
      await refreshInstances(); toast(`Created instance "${i.name}". Press Play to finish setup.`, 'ok');
    } else {
      const names = await invoke('modrinth_install', { instanceId: inst.id, projectId: pid, versionId: versionId ?? null });
      toast(names.length > 1 ? `Installed ${names[names.length - 1]} and ${names.length - 1} required ${names.length === 2 ? 'dependency' : 'dependencies'}` : `Installed ${names[0] ?? 'content'}`, 'ok');
    }
    btn.textContent = 'Installed';
  } catch (e) { toast(errMsg(e), 'error'); btn.disabled = false; btn.textContent = old; }
}

async function openProject(pid) {
  const inst = target();
  const m = modal('<div class="skel" style="height:220px"></div>', { wide: true });
  let p, vs;
  try { [p, vs] = await Promise.all([invoke('modrinth_project', { id: pid }), invoke('modrinth_versions', { id: pid })]); } catch (e) { m.close(); return toast(errMsg(e), 'error'); }
  const allowed = LOADER_COMPAT[inst.loader] || [];
  const fits = (v) => (isRust(inst) && p.project_type !== 'mod') || v.game_versions.includes(inst.mc_version) && (p.project_type !== 'mod' || v.loaders.some((l) => allowed.includes(l)));
  const tabs = { desc: () => `<div class="desc selectable">${md(p.body)}</div>`,
    gallery: () => (p.gallery?.length ? `<div class="gallery">${p.gallery.map((g) => `<figure><img loading="lazy" alt="${esc(g.title || '')}" src="${esc(g.url)}"><figcaption class="muted">${esc(g.title || '')}</figcaption></figure>`).join('')}</div>` : '<p class="muted">No screenshots.</p>'),
    versions: () => vs.slice(0, 60).map((v) => `<div class="ver" style="${fits(v) ? '' : 'opacity:.5'}"><div><b>${esc(v.name)}</b> <span class="badge ${esc(v.version_type)}">${esc(v.version_type)}</span>
        <div class="muted" style="font-size:12.5px">${esc(v.version_number)}, ${esc(v.game_versions.slice(-3).join(', '))}, ${esc(v.loaders.join(', '))}${fits(v) ? '' : ' (not compatible with this instance)'}</div></div>
        ${p.project_type === 'modpack' || fits(v) ? `<button class="btn sm" data-v="${esc(v.id)}">Install</button>` : ''}</div>`).join('') || '<p class="muted">No versions.</p>',
    changelog: () => vs.filter((v) => v.changelog).slice(0, 12).map((v) => `<h3>${esc(v.name)}</h3><div class="desc selectable">${md(v.changelog)}</div>`).join('') || '<p class="muted">No changelog.</p>' };
  m.el.innerHTML = `<div class="row" style="flex-wrap:nowrap;align-items:flex-start;gap:16px">
      ${p.icon_url ? `<img class="icon" style="width:72px;height:72px;border-radius:14px" alt="" src="${esc(p.icon_url)}">` : ''}
      <div class="grow"><h2 style="margin:0">${esc(p.title)}</h2><p class="muted">${esc(p.description)}</p>
        <div class="chips" style="margin-top:6px"><span class="chip">${fmtNum(p.downloads)} downloads</span><span class="chip">${esc(p.license?.id || '')}</span></div></div>
      <button class="btn primary" id="inst-btn">${p.project_type === 'modpack' ? 'Install as instance' : 'Install'}</button><button class="icon-btn" id="x" aria-label="Close">&#10005;</button></div>
    <div class="tabs" style="margin:16px 0" id="mt">${[['desc', 'Description'], ['gallery', 'Gallery'], ['versions', 'Versions'], ['changelog', 'Changelog']].map(([k, v], i) => `<button class="tab ${i ? '' : 'active'}" data-k="${k}">${v}</button>`).join('')}</div>
    <div id="mb"></div>`;
  const show = (k) => { $('#mb', m.el).innerHTML = tabs[k](); };
  show('desc');
  $('#x', m.el).onclick = m.close;
  $('#mt', m.el).onclick = (e) => { const k = e.target.closest('[data-k]')?.dataset.k; if (!k) return; m.el.querySelectorAll('#mt .tab').forEach((t) => t.classList.toggle('active', t.dataset.k === k)); show(k); };
  const type = M.type; M.type = p.project_type === 'modpack' ? 'modpack' : type;
  $('#inst-btn', m.el).onclick = (e) => quickInstall(inst, pid, e.currentTarget);
  $('#mb', m.el).onclick = (e) => { const b = e.target.closest('[data-v]'); if (b) quickInstall(inst, pid, b, b.dataset.v); };
}

async function installed(inst) {
  const body = $('#body');
  body.innerHTML = `<div class="page-head" style="margin-top:16px"><p class="muted">Content in <b>${esc(inst.name)}</b>. Switch a mod off to keep the file but stop it loading.</p>
    <div class="actions"><button class="btn" id="upd">Update all</button><button class="btn" id="open">Open folder</button></div></div><div class="list" id="il">${skeleton(3, 'mod')}</div>`;
  $('#open').onclick = () => invoke('open_instance_folder', { id: inst.id });
  $('#upd').onclick = async (e) => {
    e.target.disabled = true; e.target.textContent = 'Checking...';
    try { const n = (await invoke('modrinth_update_all', { instanceId: inst.id })) + (isRust(inst) ? await invoke('horizon_update_all', { instanceId: inst.id }) : 0); toast(n ? `Updated ${n} item${n > 1 ? 's' : ''}` : 'Everything is up to date', 'ok'); } catch (err) { toast(errMsg(err), 'error'); }
    installed(inst);
  };
  let items = [];
  try {
    items = await invoke('modrinth_installed', { instanceId: inst.id });
    if (isRust(inst)) items = (await invoke('horizon_installed', { instanceId: inst.id })).concat(items);
  } catch (e) { $('#il').innerHTML = `<div class="empty">${esc(errMsg(e))}</div>`; return; }
  const kindName = { mod: 'Mod', resourcepack: 'Resource pack', shader: 'Shader' };
  $('#il').innerHTML = items.length ? items.map((i, n) => `<div class="mod" style="grid-template-columns:44px 1fr auto auto;cursor:default">
      ${i.icon ? `<img class="icon" style="width:44px;height:44px" alt="" src="${esc(i.icon)}">` : '<div class="icon" style="width:44px;height:44px"></div>'}
      <div style="min-width:0"><div class="t">${esc(i.title)}</div><small class="muted">${i.source === 'horizon' ? 'Horizon mod' : kindName[i.kind]}${i.version_name ? ', ' + esc(i.version_name) : ''}</small></div>
      <button class="switch ${i.enabled ? 'on' : ''}" data-n="${n}" data-a="toggle" role="switch" aria-checked="${i.enabled}" aria-label="Enabled"></button>
      <button class="btn sm danger" data-n="${n}" data-a="rm">Remove</button></div>`).join('')
    : '<div class="empty">Nothing installed yet. Browse the tabs above to add mods, resource packs or shaders.</div>';
  $('#il').onclick = async (e) => {
    const b = e.target.closest('[data-a]'); if (!b) return; const i = items[+b.dataset.n];
    try {
      if (b.dataset.a === 'toggle' && i.source === 'horizon') { const on = await invoke('horizon_toggle', { instanceId: inst.id, id: i.project_id }); b.classList.toggle('on', on); b.setAttribute('aria-checked', on); return; }
      if (b.dataset.a === 'rm' && i.source === 'horizon') { await invoke('horizon_remove', { instanceId: inst.id, id: i.project_id }); return installed(inst); }
      if (b.dataset.a === 'toggle') { const on = await invoke('modrinth_toggle', { instanceId: inst.id, kind: i.kind, filename: i.filename }); b.classList.toggle('on', on); b.setAttribute('aria-checked', on); }
      if (b.dataset.a === 'rm') { await invoke('modrinth_remove', { instanceId: inst.id, kind: i.kind, filename: i.filename }); installed(inst); }
    } catch (err) { toast(errMsg(err), 'error'); }
  };
}
