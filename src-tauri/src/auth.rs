//! Microsoft sign-in using the official OAuth 2.0 *device code* flow, then the documented
//! Xbox Live -> XSTS -> Minecraft Services chain.
//!
//! Security model:
//!  - Passwords never touch Vesper: the user signs in on microsoft.com.
//!  - Refresh/access tokens are encrypted with AES-256-GCM before they are written to disk.
//!  - The AES key lives in the Windows Credential Manager (via the `keyring` crate), not on disk.
use crate::{download::client, error::CmdResult, instances::now, paths, settings};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const SCOPE: &str = "XboxLive.signin offline_access";
const TENANT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Clone)]
struct Secret {
    refresh_token: String,
    mc_token: String,
    mc_expires: u64,
}
#[derive(Serialize, Deserialize, Clone)]
struct Stored {
    uuid: String,
    name: String,
    enc: String,
}
#[derive(Serialize, Deserialize, Default)]
struct Store {
    active: Option<String>,
    accounts: Vec<Stored>,
}

#[derive(Serialize)]
pub struct AccountInfo {
    uuid: String,
    name: String,
    active: bool,
}
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub token: String,
}

// ---------- encrypted storage ----------

fn master_key() -> Result<Key<Aes256Gcm>> {
    let entry = keyring::Entry::new("VesperLauncher", "token-key")?;
    match entry.get_password() {
        Ok(h) => {
            let b = hex::decode(h)?;
            if b.len() != 32 {
                bail!("Stored encryption key is corrupt");
            }
            Ok(Key::<Aes256Gcm>::from_slice(&b).clone())
        }
        Err(keyring::Error::NoEntry) => {
            let k = Aes256Gcm::generate_key(OsRng);
            entry.set_password(&hex::encode(k))?;
            Ok(k)
        }
        Err(e) => Err(e.into()),
    }
}

fn seal(s: &Secret) -> Result<String> {
    let c = Aes256Gcm::new(&master_key()?);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let plain = serde_json::to_vec(s)?;
    let ct = c.encrypt(&nonce, plain.as_slice()).map_err(|_| anyhow!("Encryption failed"))?;
    let mut blob = nonce.to_vec();
    blob.extend(ct);
    Ok(B64.encode(blob))
}

fn open(enc: &str) -> Result<Secret> {
    let b = B64.decode(enc)?;
    if b.len() < 13 {
        bail!("Stored account data is corrupt");
    }
    let (n, ct) = b.split_at(12);
    let c = Aes256Gcm::new(&master_key()?);
    let pt = c.decrypt(Nonce::from_slice(n), ct).map_err(|_| anyhow!("Could not decrypt the account - sign in again"))?;
    Ok(serde_json::from_slice(&pt)?)
}

fn load_store() -> Store {
    std::fs::read(paths::accounts_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save_store(s: &Store) -> Result<()> {
    std::fs::write(paths::accounts_file(), serde_json::to_vec_pretty(s)?)?;
    Ok(())
}

// ---------- HTTP helpers (return the JSON body even on 4xx so we can read error codes) ----------

async fn post_form(url: &str, form: &[(&str, &str)]) -> Result<Value> {
    Ok(client().post(url).form(form).send().await?.json().await?)
}
async fn post_json(url: &str, body: &Value) -> Result<Value> {
    Ok(client().post(url).json(body).send().await?.json().await?)
}

fn client_id() -> Result<String> {
    let s = settings::load().ms_client_id;
    if !s.trim().is_empty() {
        return Ok(s.trim().to_string());
    }
    if let Some(c) = option_env!("VESPER_MS_CLIENT_ID") {
        if !c.is_empty() {
            return Ok(c.to_string());
        }
    }
    bail!("No Microsoft client ID is set. Add your Azure app's Application (client) ID in Settings, under Microsoft sign-in.")
}

// ---------- Xbox / Minecraft chain ----------

struct Chain {
    name: String,
    uuid: String,
    mc_token: String,
    mc_expires: u64,
}

async fn xbox_chain(ms_access: &str) -> Result<Chain> {
    let xbl = post_json(
        "https://user.auth.xboxlive.com/user/authenticate",
        &json!({"Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")}, "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"}),
    )
    .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live sign-in failed"))?;
    let xsts = post_json(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        &json!({"Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]}, "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"}),
    )
    .await?;
    if let Some(x) = xsts["XErr"].as_u64() {
        bail!(match x {
            2148916233 => "This Microsoft account has no Xbox profile. Sign in once at minecraft.net to create one.".to_string(),
            2148916235 => "Xbox Live is not available in your country or region.".to_string(),
            2148916238 => "This is a child account. An adult must add it to a Microsoft family before it can play.".to_string(),
            _ => format!("Xbox sign-in was refused (code {x})."),
        });
    }
    let xtoken = xsts["Token"].as_str().ok_or_else(|| anyhow!("XSTS token missing"))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| anyhow!("XSTS user hash missing"))?;
    let mc = post_json("https://api.minecraftservices.com/authentication/login_with_xbox", &json!({"identityToken": format!("XBL3.0 x={uhs};{xtoken}")})).await?;
    let token = mc["access_token"].as_str().ok_or_else(|| anyhow!("Minecraft sign-in was refused: {}", mc["errorMessage"].as_str().unwrap_or("unknown error")))?.to_string();
    let expires = now() + mc["expires_in"].as_u64().unwrap_or(86400).saturating_sub(60);
    let prof = client().get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&token).send().await?;
    if prof.status().as_u16() == 404 {
        bail!("This account does not own Minecraft: Java Edition.");
    }
    let prof: Value = prof.error_for_status()?.json().await?;
    Ok(Chain {
        name: prof["name"].as_str().unwrap_or("Player").to_string(),
        uuid: prof["id"].as_str().ok_or_else(|| anyhow!("Profile has no id"))?.to_string(),
        mc_token: token,
        mc_expires: expires,
    })
}

// ---------- commands ----------

#[derive(Serialize)]
pub struct DeviceCode {
    user_code: String,
    verification_uri: String,
    device_code: String,
    interval: u64,
    expires_in: u64,
}

#[tauri::command]
pub async fn login_start() -> CmdResult<DeviceCode> {
    let cid = client_id()?;
    CANCEL.store(false, Ordering::SeqCst);
    let r = post_form(&format!("{TENANT}/devicecode"), &[("client_id", cid.as_str()), ("scope", SCOPE)]).await?;
    if let Some(e) = r["error"].as_str() {
        return Err(anyhow!("Microsoft refused the request: {} ({e})", r["error_description"].as_str().unwrap_or("")).into());
    }
    Ok(DeviceCode {
        user_code: r["user_code"].as_str().unwrap_or_default().into(),
        verification_uri: r["verification_uri"].as_str().unwrap_or("https://microsoft.com/link").into(),
        device_code: r["device_code"].as_str().unwrap_or_default().into(),
        interval: r["interval"].as_u64().unwrap_or(5),
        expires_in: r["expires_in"].as_u64().unwrap_or(900),
    })
}

#[tauri::command]
pub fn login_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Poll until the user finishes signing in on microsoft.com.
#[tauri::command]
pub async fn login_finish(device_code: String, interval: u64, expires_in: u64) -> CmdResult<AccountInfo> {
    let cid = client_id()?;
    let deadline = Instant::now() + Duration::from_secs(expires_in);
    let mut every = interval.max(3);
    loop {
        tokio::time::sleep(Duration::from_secs(every)).await;
        if CANCEL.load(Ordering::SeqCst) {
            return Err(anyhow!("Sign-in cancelled").into());
        }
        if Instant::now() > deadline {
            return Err(anyhow!("The sign-in code expired. Start again.").into());
        }
        let r = post_form(
            &format!("{TENANT}/token"),
            &[("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", cid.as_str()), ("device_code", device_code.as_str())],
        )
        .await?;
        match r["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                every += 5;
                continue;
            }
            Some(e) => return Err(anyhow!("Sign-in failed: {} ({e})", r["error_description"].as_str().unwrap_or("")).into()),
            None => {}
        }
        let access = r["access_token"].as_str().ok_or_else(|| anyhow!("No access token returned"))?;
        let refresh = r["refresh_token"].as_str().ok_or_else(|| anyhow!("No refresh token returned (is offline_access allowed?)"))?;
        let c = xbox_chain(access).await?;
        let mut store = load_store();
        let enc = seal(&Secret { refresh_token: refresh.into(), mc_token: c.mc_token, mc_expires: c.mc_expires })?;
        store.accounts.retain(|a| a.uuid != c.uuid);
        store.accounts.push(Stored { uuid: c.uuid.clone(), name: c.name.clone(), enc });
        store.active = Some(c.uuid.clone());
        save_store(&store)?;
        return Ok(AccountInfo { uuid: c.uuid, name: c.name, active: true });
    }
}

#[tauri::command]
pub fn list_accounts() -> Vec<AccountInfo> {
    let s = load_store();
    let active = s.active.clone().or_else(|| s.accounts.first().map(|a| a.uuid.clone()));
    s.accounts.iter().map(|a| AccountInfo { uuid: a.uuid.clone(), name: a.name.clone(), active: Some(&a.uuid) == active.as_ref() }).collect()
}

#[tauri::command]
pub fn set_active_account(uuid: String) -> CmdResult<()> {
    let mut s = load_store();
    if !s.accounts.iter().any(|a| a.uuid == uuid) {
        return Err(anyhow!("Unknown account").into());
    }
    s.active = Some(uuid);
    Ok(save_store(&s)?)
}

#[tauri::command]
pub fn remove_account(uuid: String) -> CmdResult<()> {
    let mut s = load_store();
    s.accounts.retain(|a| a.uuid != uuid);
    if s.active.as_deref() == Some(uuid.as_str()) {
        s.active = s.accounts.first().map(|a| a.uuid.clone());
    }
    Ok(save_store(&s)?)
}

/// A valid Minecraft session for the active account; refreshes tokens when needed.
pub async fn session() -> Result<Session> {
    let mut store = load_store();
    let id = store.active.clone().or_else(|| store.accounts.first().map(|a| a.uuid.clone())).ok_or_else(|| anyhow!("Sign in with a Microsoft account first (Accounts tab)."))?;
    let pos = store.accounts.iter().position(|a| a.uuid == id).ok_or_else(|| anyhow!("Active account not found"))?;
    let mut sec = open(&store.accounts[pos].enc)?;
    let mut name = store.accounts[pos].name.clone();
    if now() + 30 >= sec.mc_expires {
        let cid = client_id()?;
        let r = post_form(&format!("{TENANT}/token"), &[("grant_type", "refresh_token"), ("client_id", cid.as_str()), ("refresh_token", sec.refresh_token.as_str()), ("scope", SCOPE)]).await?;
        let access = r["access_token"].as_str().ok_or_else(|| anyhow!("Your Microsoft session expired. Remove the account and sign in again."))?;
        if let Some(rt) = r["refresh_token"].as_str() {
            sec.refresh_token = rt.into();
        }
        let c = xbox_chain(access).await?;
        sec.mc_token = c.mc_token;
        sec.mc_expires = c.mc_expires;
        name = c.name;
        store.accounts[pos].name = name.clone();
        store.accounts[pos].enc = seal(&sec)?;
        save_store(&store)?;
    }
    Ok(Session { name, uuid: id, token: sec.mc_token })
}
