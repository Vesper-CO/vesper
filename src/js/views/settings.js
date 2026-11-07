import { invoke } from '../api.js';
import { S, applyTheme } from '../state.js';
import { $, esc, toast, errMsg, ramLabel } from '../util.js';
import { promptUpdate } from '../shell.js';

export async function render(el) {
  const s = S.settings;
  el.innerHTML = `<div class="page" style="max-width:720px">
    <div class="page-head"><div><h1>Settings</h1></div></div>
    <div class="card"><h3>Appearance</h3>
      <div class="row"><button class="btn ${s.theme === 'dark' ? 'primary' : ''}" data-theme="dark">Dark</button><button class="btn ${s.theme === 'light' ? 'primary' : ''}" data-theme="light">Light</button></div></div>
    <h2>Game</h2>
    <div class="card">
      <label class="field"><span>Default memory for new instances: <b id="ramv">${ramLabel(s.default_ram_mb)}</b></span><input type="range" id="ram" min="1024" max="16384" step="512" value="${s.default_ram_mb}"></label>
      <label class="field"><span>Custom java.exe (optional)</span><input type="text" id="java" value="${esc(s.java_path)}" placeholder="Leave empty to use the Java that Mojang provides"></label>
    </div>
    <h2>Updates</h2>
    <div class="card"><div class="row" style="justify-content:space-between"><span>Check for updates when Vesper starts</span><button class="switch ${s.auto_update ? 'on' : ''}" id="auto" role="switch" aria-checked="${s.auto_update}" aria-label="Check for updates on start"></button></div>
      <div class="row"><button class="btn" id="check">Check for updates</button><button class="btn" id="data">Open data folder</button></div></div>
    <p class="muted" style="margin-top:22px">Vesper is an unofficial launcher. It is not approved by or associated with Mojang or Microsoft. Minecraft game files are downloaded from Mojang's servers on your computer and are never redistributed by Vesper.</p>
  </div>`;
  const save = async () => { try { await invoke('save_settings', { settings: S.settings }); } catch (e) { toast(errMsg(e), 'error'); } };
  el.querySelectorAll('[data-theme]').forEach((b) => b.onclick = async () => { S.settings.theme = b.dataset.theme; applyTheme(); await save(); render(el); });
  $('#ram').oninput = (e) => { S.settings.default_ram_mb = +e.target.value; $('#ramv').textContent = ramLabel(+e.target.value); };
  $('#ram').onchange = save;
  $('#java').onchange = (e) => { S.settings.java_path = e.target.value; save(); };
  $('#auto').onclick = (e) => { S.settings.auto_update = !S.settings.auto_update; e.target.classList.toggle('on', S.settings.auto_update); e.target.setAttribute('aria-checked', S.settings.auto_update); save(); };
  $('#data').onclick = () => invoke('open_data_folder');
  $('#check').onclick = async () => {
    try { const info = await invoke('check_update'); if (!info) return toast('Vesper is up to date', 'ok'); promptUpdate(info); } catch (e) { toast(errMsg(e), 'error'); }
  };
}
