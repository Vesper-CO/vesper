import { invoke } from '../api.js';
import { S, refreshAccounts } from '../state.js';
import { $, esc, ask, modal, toast, errMsg } from '../util.js';
import { renderChip } from '../shell.js';

export async function render(el) {
  await refreshAccounts();
  renderChip();
  el.innerHTML = `<div class="page">
    <div class="page-head"><div><h1>Accounts</h1><p class="muted">You sign in on Microsoft's own page. Vesper never sees your password.</p></div>
    <div class="actions"><button class="btn primary" id="add">Add account</button></div></div>
    ${S.accounts.length ? `<div class="grid">${S.accounts.map((a) => `<div class="card">
      <div class="row" style="flex-wrap:nowrap"><img class="avatar" style="width:44px;height:44px" alt="" src="https://mc-heads.net/avatar/${a.uuid}/88">
        <div class="grow"><h3>${esc(a.name)}</h3><small class="muted">${a.active ? 'Playing with this account' : 'Available'}</small></div></div>
      <div class="actions">${a.active ? '' : `<button class="btn sm" data-a="use" data-id="${a.uuid}">Use this account</button>`}<button class="btn sm danger" data-a="rm" data-id="${a.uuid}">Remove</button></div></div>`).join('')}</div>`
      : `<div class="empty"><b>No account yet</b><span>Add the Microsoft account that owns Minecraft: Java Edition.</span><button class="btn primary" id="add2">Add account</button></div>`}
  </div>`;
  const add = () => signIn(() => render(el));
  $('#add').onclick = add; $('#add2')?.addEventListener('click', add);
  el.onclick = async (e) => {
    const b = e.target.closest('[data-a]'); if (!b) return;
    try {
      if (b.dataset.a === 'use') await invoke('set_active_account', { uuid: b.dataset.id });
      if (b.dataset.a === 'rm') { if (!(await ask('Remove this account from Vesper? You can sign in again later.', 'Remove'))) return; await invoke('remove_account', { uuid: b.dataset.id }); }
      render(el);
    } catch (err) { toast(errMsg(err), 'error'); }
  };
}

async function signIn(done) {
  let code;
  try { code = await invoke('login_start'); } catch (e) { toast(errMsg(e), 'error'); return; }
  const m = modal(`<h2>Sign in with Microsoft</h2>
    <p>Open the Microsoft page and enter this code:</p><div class="code selectable">${esc(code.user_code)}</div>
    <div class="row" style="justify-content:center"><button class="btn primary" id="open">Open microsoft.com/link</button><button class="btn" id="copy">Copy code</button></div>
    <p class="muted" style="margin-top:14px;text-align:center">Waiting for you to finish. This window updates by itself.</p>
    <div class="foot"><button class="btn" id="cancel">Cancel</button></div>`);
  m.el.querySelector('#open').onclick = () => invoke('open_url', { url: code.verification_uri });
  m.el.querySelector('#copy').onclick = () => navigator.clipboard.writeText(code.user_code).then(() => toast('Code copied', 'ok'));
  m.el.querySelector('#cancel').onclick = () => { invoke('login_cancel'); m.close(); };
  invoke('open_url', { url: code.verification_uri }).catch(() => {});
  try {
    const a = await invoke('login_finish', { deviceCode: code.device_code, interval: code.interval, expiresIn: code.expires_in });
    m.close(); toast(`Signed in as ${a.name}`, 'ok'); await refreshAccounts(); renderChip(); done();
  } catch (e) { m.close(); if (!/cancelled/i.test(errMsg(e))) toast(errMsg(e), 'error'); }
}
