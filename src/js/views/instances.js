import { invoke, pickFile } from '../api.js';
import { S, mcVersions, refreshInstances } from '../state.js';
import { $, esc, ask, modal, toast, errMsg, ICON, LOADER_NAMES, ramLabel, ago, skeleton } from '../util.js';
import { go, play, syncPlayUI } from '../shell.js';

export async function render(el) {
  el.innerHTML = `<div class="page">
    <div class="page-head"><div><h1>Instances</h1><p class="muted">Each instance keeps its own version, loader, mods and settings.</p></div>
    <div class="actions"><button class="btn" id="imp">Import .mrpack</button><button class="btn primary" id="add">New instance</button></div></div>
    <div class="grid" id="grid">${skeleton(3)}</div></div>`;
  $('#add').onclick = () => form(null, () => render(el));
  $('#imp').onclick = async () => {
    const path = await pickFile([{ name: 'Modrinth modpack', extensions: ['mrpack'] }]);
    if (!path) return;
    toast('Importing modpack...');
    try { const i = await invoke('import_mrpack', { path }); toast(`Imported ${i.name}`, 'ok'); await refreshInstances(); render(el); } catch (e) { toast(errMsg(e), 'error'); }
  };
  await refreshInstances();
  const grid = $('#grid');
  if (!S.instances.length) { grid.outerHTML = `<div class="empty"><b>No instances yet</b><span>Create one to pick a Minecraft version and loader.</span><button class="btn primary" id="add2">New instance</button></div>`; $('#add2').onclick = () => form(null, () => render(el)); return; }
  grid.innerHTML = S.instances.map((i) => `<div class="card">
      <div class="row" style="justify-content:space-between;flex-wrap:nowrap"><h3>${esc(i.name)}</h3>
        <div class="row" style="gap:0;flex-wrap:nowrap"><button class="icon-btn" data-a="folder" data-id="${i.id}" title="Open folder">${ICON.folder}</button>
        <button class="icon-btn" data-a="edit" data-id="${i.id}" title="Edit">${ICON.edit}</button>
        <button class="icon-btn" data-a="del" data-id="${i.id}" title="Delete">${ICON.trash}</button></div></div>
      <div class="chips"><span class="chip accent">${LOADER_NAMES[i.loader]}</span><span class="chip">${esc(i.mc_version)}</span>${i.loader === 'rust' ? '<span class="chip">No Java</span>' : `<span class="chip">${ramLabel(i.ram_mb)}</span>`}</div>
      <small class="muted">${ago(i.last_played)}</small>
      <div class="actions"><button class="btn primary sm" data-play="${i.id}" data-a="play" data-id="${i.id}">Play</button><button class="btn sm" data-a="content" data-id="${i.id}">Mods and packs</button>
        ${i.loader === 'rust' ? '' : `<button class="btn sm" data-a="boost" data-id="${i.id}" title="Install performance mods">Boost FPS</button><button class="btn sm" data-a="hud" data-id="${i.id}" title="In-game HUD: FPS, CPS, keystrokes... (works on every loader)">HUD: ${i.hud ? 'on' : 'off'}</button>`}</div>
    </div>`).join('');
  grid.onclick = async (e) => {
    const b = e.target.closest('[data-a]'); if (!b) return;
    const id = b.dataset.id, inst = S.instances.find((i) => i.id === id);
    try {
      if (b.dataset.a === 'play') play(id);
      if (b.dataset.a === 'folder') await invoke('open_instance_folder', { id });
      if (b.dataset.a === 'edit') form(inst, () => render(el));
      if (b.dataset.a === 'content') { S.target = id; go('mods'); }
      if (b.dataset.a === 'boost' && await ask(`Install performance mods (such as Sodium and Lithium) into "${inst.name}"? They are free, from Modrinth, and can be removed in Mods and packs.`, 'Install')) {
        toast('Installing performance mods...');
        const r = await invoke('optimize_instance', { id });
        toast(`Installed ${r.installed.length} mod${r.installed.length === 1 ? '' : 's'}${r.unavailable.length ? `. Not available for ${inst.mc_version}: ${r.unavailable.join(', ')}` : ''}`, 'ok');
      }
      if (b.dataset.a === 'hud') {
        if (inst.hud) { await invoke('hud_set', { id, enabled: false }); toast('In-game HUD turned off for this instance', 'ok'); }
        else {
          await invoke('hud_set', { id, enabled: true });
          toast('In-game HUD turned on. Style it on the In-game HUD page.', 'ok');
          if ((inst.loader === 'fabric' || inst.loader === 'quilt') && inst.mc_version === '1.21.4'
            && await ask('Also install the Fabric add-on? It adds coordinates, ping, direction, armor and potion effects.', 'Install')) {
            toast('Installing the add-on...'); await invoke('hud_install', { id }); toast('Add-on installed', 'ok');
          }
        }
        await refreshInstances(); render(el);
      }
      if (b.dataset.a === 'del' && await ask(`Delete "${inst.name}" with all its worlds and mods? This cannot be undone.`, 'Delete')) { await invoke('delete_instance', { id }); toast('Instance deleted', 'ok'); render(el); }
    } catch (err) { toast(errMsg(err), 'error'); }
  };
  syncPlayUI();
}

const RUST_NOTE = 'Rust runs Minecraft 26.3 natively, rewritten in Rust with no Java, so memory and Java settings do not apply. Mods come from the Horizon library; resource packs and shaders from Modrinth. You need a graphics card with Vulkan or DirectX 12. The first launch downloads the Rust build and prepares the game from your own Minecraft account, which takes a few minutes.';
const typeOf = (v) => (v.type === 'release' ? 'release' : v.type === 'snapshot' ? 'snapshot' : 'old');

async function form(inst, done) {
  const versions = await mcVersions();
  const cur = inst ? versions.find((v) => v.id === inst.mc_version) : null;
  const m = modal(`<h2>${inst ? 'Edit instance' : 'New instance'}</h2>
    <label class="field"><span>Name</span><input type="text" id="f-name" maxlength="40" value="${esc(inst?.name || '')}" placeholder="My survival world"></label>
    <div class="row" id="r-ver"><label class="field grow"><span>Minecraft</span><select id="f-ver"></select></label>
      <label class="field" style="width:150px"><span>Show</span><select id="f-type"><option value="release">Releases</option><option value="snapshot">Snapshots</option><option value="old">Old versions</option></select></label></div>
    <div class="row"><label class="field grow"><span>Mod loader</span><select id="f-loader">${Object.entries(LOADER_NAMES).map(([k, v]) => `<option value="${k}">${v}</option>`).join('')}</select></label>
      <label class="field grow" id="l-lv"><span>Loader version</span><select id="f-lv"></select></label></div>
    <p class="muted" id="f-note" style="margin:-6px 0 12px"></p>
    <label class="field" id="l-ram"><span>Memory: <b id="f-ramv"></b></span><input type="range" id="f-ram" min="1024" max="16384" step="512"></label>
    <label class="field" id="l-perf"><span>Performance</span><select id="f-perf"><option value="auto">Auto (recommended)</option><option value="fps">Max FPS</option><option value="lowram">Low memory</option><option value="off">Off: use only my own settings</option></select></label>
    <p class="muted" id="f-perfnote" style="margin:-6px 0 12px"></p>
    <label class="field" id="l-jvm"><span>Extra JVM arguments</span><textarea id="f-jvm" rows="2" placeholder="Added last, so they override the performance settings"></textarea></label>
    <div class="foot"><button class="btn" id="f-cancel">Cancel</button><button class="btn primary" id="f-save">${inst ? 'Save' : 'Create'}</button></div>`);
  const $m = (s) => m.el.querySelector(s);
  $m('#f-type').value = cur ? typeOf(cur) : 'release';
  $m('#f-loader').value = inst?.loader || 'vanilla';
  $m('#f-ram').value = inst?.ram_mb || S.settings.default_ram_mb || 4096;
  $m('#f-jvm').value = inst?.jvm_args || '';
  $m('#f-perf').value = inst?.perf || 'auto';
  // A plain-language preview of what the launcher will do with these settings.
  let perfTimer = 0, rec = 0;
  const perfNote = () => {
    clearTimeout(perfTimer);
    perfTimer = setTimeout(async () => {
      const loader = $m('#f-loader').value; if (loader === 'rust') return;
      try {
        const r = await invoke('perf_info', { id: inst?.id ?? null, mcVersion: $m('#f-ver').value, loader, profile: $m('#f-perf').value, ramMb: +$m('#f-ram').value });
        rec = r.recommended_mb;
        const note = $m('#f-perfnote'); if (!note.isConnected) return;
        note.innerHTML = `${r.plan.notes.map(esc).join(' ')} Suggested for this PC (${ramLabel(r.system_mb)} installed${r.mods ? `, ${r.mods} mods` : ''}): <b>${ramLabel(rec)}</b>${+$m('#f-ram').value === rec ? '' : ' <button class="btn sm" id="f-rec" type="button">Use it</button>'}`;
        const b = $m('#f-rec'); if (b) b.onclick = () => { $m('#f-ram').value = rec; ram(); perfNote(); };
      } catch { /* the preview is optional */ }
    }, 150);
  };
  const ram = () => { $m('#f-ramv').textContent = ramLabel(+$m('#f-ram').value); perfNote(); };
  $m('#f-ram').oninput = ram; $m('#f-perf').onchange = perfNote; ram();

  const fillVersions = () => {
    const t = $m('#f-type').value;
    $m('#f-ver').innerHTML = versions.filter((v) => typeOf(v) === t).map((v) => `<option>${esc(v.id)}</option>`).join('');
    if (inst && cur && typeOf(cur) === t) $m('#f-ver').value = inst.mc_version;
  };
  let token = 0;
  const fillLoader = async () => {
    const loader = $m('#f-loader').value, mc = $m('#f-ver').value, my = ++token;
    const rust = loader === 'rust';
    for (const s of ['#r-ver', '#l-lv', '#l-ram', '#l-jvm', '#l-perf', '#f-perfnote']) $m(s).style.display = rust ? 'none' : '';
    if (rust) { $m('#f-save').disabled = false; $m('#f-note').textContent = RUST_NOTE; return; }
    $m('#f-lv').disabled = loader === 'vanilla';
    $m('#f-save').disabled = false;
    if (loader === 'vanilla') { $m('#f-lv').innerHTML = '<option value="">Not needed</option>'; $m('#f-note').textContent = ''; return; }
    $m('#f-lv').innerHTML = '<option>Checking...</option>'; $m('#f-save').disabled = true;
    let list = [];
    try { list = await invoke('loader_versions', { loader, mc }); } catch { /* offline */ }
    if (my !== token) return;
    if (!list.length) { $m('#f-lv').innerHTML = '<option value="">None</option>'; $m('#f-note').textContent = `${LOADER_NAMES[loader]} is not available for Minecraft ${mc}. Pick another version or loader.`; return; }
    $m('#f-note').textContent = '';
    $m('#f-lv').innerHTML = list.map((v, i) => `<option value="${esc(v)}">${esc(v)}${i === 0 ? ' (recommended)' : ''}</option>`).join('');
    if (inst && inst.loader === loader && list.includes(inst.loader_version)) $m('#f-lv').value = inst.loader_version;
    $m('#f-save').disabled = false;
  };
  $m('#f-type').onchange = () => { fillVersions(); fillLoader(); perfNote(); };
  $m('#f-ver').onchange = () => { fillLoader(); perfNote(); };
  $m('#f-loader').onchange = () => { fillLoader(); perfNote(); };
  fillVersions(); fillLoader();

  $m('#f-cancel').onclick = m.close;
  $m('#f-save').onclick = async () => {
    const v = { name: $m('#f-name').value, mc: $m('#f-ver').value, loader: $m('#f-loader').value, lv: $m('#f-lv').value, ram: +$m('#f-ram').value, jvm: $m('#f-jvm').value, perf: $m('#f-perf').value };
    try {
      if (inst) await invoke('update_instance', { inst: { ...inst, name: v.name.trim() || inst.name, mc_version: v.mc, loader: v.loader, loader_version: v.lv, ram_mb: v.ram, jvm_args: v.jvm, perf: v.perf } });
      else {
        const made = await invoke('create_instance', { name: v.name, mcVersion: v.mc, loader: v.loader, loaderVersion: v.lv, ramMb: v.ram });
        if (v.perf !== 'auto') await invoke('update_instance', { inst: { ...made, perf: v.perf } });
      }
      m.close(); await refreshInstances(); done();
    } catch (e) { toast(errMsg(e), 'error'); }
  };
}
