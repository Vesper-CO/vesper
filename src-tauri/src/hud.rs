//! The in-game HUD (FPS, CPS, keystrokes...). The look of every element is edited in the launcher's
//! "In-game HUD" page and saved to one file, `hud.json`. Before each launch that file is copied into
//! the instance as `config/vesper-hud.json`, where the Vesper HUD mod (see /hud-mod) reads it and
//! re-reads it whenever it changes, so edits show up in a running game.
use crate::{download::{client, download_one, Job}, error::CmdResult, instances, paths};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::AppHandle;

/// Minecraft versions a HUD mod build is published for.
pub const HUD_VERSIONS: &[&str] = &["1.21.4"];
const RELEASE: &str = "https://github.com/Vesper-CO/vesper/releases/download/hud-latest";
const MAX_CONFIG: usize = 256 * 1024;

fn hud_file() -> std::path::PathBuf { paths::root().join("hud.json") }

/// The saved HUD settings, or `null` when nothing was saved yet (the UI then uses its defaults).
#[tauri::command]
pub fn get_hud() -> Value {
    std::fs::read(hud_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

#[tauri::command]
pub fn save_hud(hud: Value) -> CmdResult<()> {
    if !hud.is_object() {
        return Err(anyhow!("HUD settings must be an object").into());
    }
    let bytes = serde_json::to_vec_pretty(&hud)?;
    if bytes.len() > MAX_CONFIG {
        return Err(anyhow!("HUD settings are too large").into());
    }
    std::fs::write(hud_file(), bytes)?;
    Ok(())
}

/// Copy the saved HUD settings into an instance's config folder (called before launching).
pub fn sync(id: &str) -> Result<()> {
    let Ok(bytes) = std::fs::read(hud_file()) else { return Ok(()) };
    let dir = paths::game_dir(id).join("config");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("vesper-hud.json"), bytes).context("Could not write the HUD settings")?;
    Ok(())
}

fn remove_old_jars(mods: &std::path::Path) {
    if let Ok(rd) = std::fs::read_dir(mods) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("vesper-hud-") && (n.ends_with(".jar") || n.ends_with(".jar.disabled")) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Install the HUD mod (and Fabric API, which it needs) into a Fabric or Quilt instance.
#[tauri::command]
pub async fn hud_install(app: AppHandle, id: String) -> CmdResult<String> {
    let mut inst = instances::load(&id)?;
    if inst.loader != "fabric" && inst.loader != "quilt" {
        return Err(anyhow!("The in-game HUD needs a Fabric or Quilt instance.").into());
    }
    if !HUD_VERSIONS.contains(&inst.mc_version.as_str()) {
        return Err(anyhow!("The in-game HUD is not available for Minecraft {} yet (it supports {}).", inst.mc_version, HUD_VERSIONS.join(", ")).into());
    }
    crate::modrinth::modrinth_install(app.clone(), id.clone(), "fabric-api".into(), None).await?;

    let name = format!("vesper-hud-{}.jar", inst.mc_version);
    let sum_text = client().get(format!("{RELEASE}/{name}.sha256")).send().await.and_then(|r| r.error_for_status()).context("The HUD mod is not published yet")?.text().await?;
    let want = sum_text.split_whitespace().next().unwrap_or("").to_lowercase();
    if want.len() != 64 || !want.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("The HUD mod's checksum file is invalid").into());
    }
    let mods = paths::game_dir(&id).join("mods");
    std::fs::create_dir_all(&mods)?;
    let tmp = paths::tmp().join(&name);
    std::fs::create_dir_all(paths::tmp())?;
    download_one(&Job { url: format!("{RELEASE}/{name}"), path: tmp.clone(), sha1: None, size: None }).await?;
    let got = hex::encode(Sha256::digest(std::fs::read(&tmp)?));
    if got != want {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("The downloaded HUD mod did not match its checksum, so it was not installed").into());
    }
    remove_old_jars(&mods);
    std::fs::copy(&tmp, mods.join(&name))?;
    let _ = std::fs::remove_file(&tmp);
    sync(&id)?;
    inst.hud = true;
    instances::save(&inst)?;
    Ok(name)
}

/// Turn the HUD off for an instance: the mod file is removed, the settings file stays.
#[tauri::command]
pub fn hud_remove(id: String) -> CmdResult<()> {
    let mut inst = instances::load(&id)?;
    remove_old_jars(&paths::game_dir(&id).join("mods"));
    inst.hud = false;
    instances::save(&inst)?;
    Ok(())
}
