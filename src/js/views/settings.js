import { invoke } from '../api.js';
import { S, applyTheme } from '../state.js';
import { $, esc, toast, errMsg, ramLabel } from '../util.js';
import { promptUpdate } from '../shell.js';

const ico = (d) => `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
const MOON = ico('<path d="M21 13A9 9 0 1111 3a7 7 0 0010 10z"/>');
const SUN = ico('<circle cx="12" cy="12" r="4.2"/><path d="M12 2.5v2.4M12 19.1v2.4M2.5 12h2.4M19.1 12h2.4M5.3 5.3l1.7 1.7M17 17l1.7 1.7M5.3 18.7L7 17M17 7l1.7-1.7"/>');

export async function render(el) {
  const s = S.settings;
  el.innerHTML = `<div class="page" style="max-width:720px">
    <div class="page-head"><div><h1>Settings</h1></div></div>
    <div class="card"><h3>Appearance</h3>
      <div class="row"><button class="btn ${s.theme === 'dark' ? 'primary' : ''}" data-theme="dark" style="display:inline-flex;align-items:center;gap:8px">${MOON}Dark</button><button class="btn ${s.theme === 'light' ? 'primary' : ''}" data-theme="light" style="display:inline-flex;align-items:center;gap:8px">${SUN}Light</button></div>
      <p class="muted" style="margin:0">Dark is a starry night with a moon. Light is a sunny day.</p>
      <div class="row" style="justify-content:space-between"><span>Christmas theme<br><small class="muted">Snow, Christmas trees, red and green. Works with dark and light.</small></span><button class="switch ${s.christmas ? 'on' : ''}" id="xmas" role="switch" aria-checked="${!!s.christmas}"></button></div></div>
    <h2>Game</h2>
    <div class="card">
      <label class="field"><span>Default memory for new instances: <b id="ramv">${ramLabel(s.default_ram_mb)}</b></span><input type="range" id="ram" min="1024" max="16384" step="512" value="${s.default_ram_mb}"></label>
      <label class="field"><span>Custom java.exe (optional)</span><input type="text" id="java" value="${esc(s.java_path)}" placeholder="Leave empty to use the Java that Mojang provides"></label>
    </div>
    <h2>Discord</h2>
    <div class="card" id="dc"></div>
    <h2>Updates</h2>
    <div class="card"><div class="row" style="justify-content:space-between"><span>Check for updates when Vesper starts</span><button class="switch ${s.auto_update ? 'on' : ''}" id="auto" role="switch" aria-checked="${s.auto_update}" aria-label="Check for updates on start"></button></div>
      <div class="row"><button class="btn" id="check">Check for updates</button><button class="btn" id="data">Open data folder</button></div></div>
    <p class="muted" style="margin-top:22px">Vesper is an unofficial launcher. It is not approved by or associated with Mojang or Microsoft. Minecraft game files are downloaded from Mojang's servers on your computer and are never redistributed by Vesper.</p>
  </div>`;
  const save = async () => { try { await invoke('save_settings', { settings: S.settings }); } catch (e) { toast(errMsg(e), 'error'); } };
  el.querySelectorAll('[data-theme]').forEach((b) => b.onclick = async () => { S.settings.theme = b.dataset.theme; applyTheme(); await save(); render(el); });
  $('#xmas').onclick = async () => { S.settings.christmas = !S.settings.christmas; applyTheme(); await save(); render(el); };
  $('#ram').oninput = (e) => { S.settings.default_ram_mb = +e.target.value; $('#ramv').textContent = ramLabel(+e.target.value); };
  $('#ram').onchange = save;
  $('#java').onchange = (e) => { S.settings.java_path = e.target.value; save(); };
  $('#auto').onclick = (e) => { S.settings.auto_update = !S.settings.auto_update; e.target.classList.toggle('on', S.settings.auto_update); e.target.setAttribute('aria-checked', S.settings.auto_update); save(); };
  const dc = $('#dc');
  let avail = false;
  try { avail = await invoke('presence_available'); } catch { /* an older build */ }
  dc.innerHTML = avail
    ? `<div class="row" style="justify-content:space-between"><span>Show what I am playing on Discord</span><button class="switch ${s.discord_presence ? 'on' : ''}" id="dcs" role="switch" aria-checked="${!!s.discord_presence}" aria-label="Discord Rich Presence"></button></div>
       <p class="muted" style="margin:8px 0 0">Shows the instance name and Minecraft version, and in Vesper's Rust client whether you are in singleplayer or multiplayer. Never server addresses or accounts.</p>`
    : '<p class="muted" style="margin:0">Discord Rich Presence is not set up in this build of Vesper: it was made without a Discord application ID. Add the DISCORD_APP_ID variable to the repository and release again.</p>';
  const dcs = $('#dcs');
  if (dcs) dcs.onclick = (e) => { S.settings.discord_presence = !S.settings.discord_presence; e.target.classList.toggle('on', S.settings.discord_presence); e.target.setAttribute('aria-checked', S.settings.discord_presence); save(); };
  $('#data').onclick = () => invoke('open_data_folder');
  $('#check').onclick = async () => {
    try { const info = await invoke('check_update'); if (!info) return toast('Vesper is up to date', 'ok'); promptUpdate(info); } catch (e) { toast(errMsg(e), 'error'); }
  };
}
