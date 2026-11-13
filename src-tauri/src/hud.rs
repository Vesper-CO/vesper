//! The in-game HUD (FPS, CPS, keystrokes...). The look of every element is edited in the launcher's
//! "In-game HUD" page and saved to one file, `hud.json`. Before each launch that file is copied into
//! the instance as `config/vesper-hud.json`, where the Vesper HUD mod (see /hud-mod) reads it and
//! re-reads it whenever it changes, so edits show up in a running game.
use crate::{download::{client, download_one, Job}, error::CmdResult, instances, paths};
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Minecraft versions a HUD mod build is published for.
pub const HUD_VERSIONS: &[&str] = &["1.21.4"];
const RELEASE: &str = "https://github.com/Vesper-CO/vesper/releases/download/hud-latest";
const MAX_CONFIG: usize = 256 * 1024;

/// The universal HUD agent (see /hud-agent), built into the launcher so it needs no download.
const AGENT_JAR: &[u8] = include_bytes!("../resources/vesper-hud-agent.jar");

/// Write the agent jar to disk (named by its hash, so a new launcher version never reuses a stale one).
fn agent_path() -> Result<PathBuf> {
    let dir = paths::root().join("agent");
    let path = dir.join(format!("vesper-hud-agent-{}.jar", &hex::encode(Sha256::digest(AGENT_JAR))[..12]));
    if !path.exists() {
        std::fs::create_dir_all(&dir)?;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with("vesper-hud-agent-") {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        std::fs::write(&path, AGENT_JAR).context("Could not write the HUD agent")?;
    }
    Ok(path)
}

/// Mojang's trimmed Java runtimes list their modules in a `release` file. Agents need `java.instrument`;
/// without it the JVM refuses to start, so we check first. (Java 8 has no module list and always has it.)
fn runtime_has_instrument(java: &Path) -> bool {
    let release = java.parent().and_then(|p| p.parent()).map(|p| p.join("release"));
    match release.and_then(|r| std::fs::read_to_string(r).ok()) {
        Some(text) => match text.lines().find(|l| l.starts_with("MODULES=")) {
            Some(m) => m.contains("java.instrument"),
            None => true,
        },
        None => true,
    }
}

/// JVM arguments that start the HUD agent for an instance (called by the launcher before starting the game).
pub fn agent_args(id: &str, mc_version: &str, java: &Path) -> Result<Vec<String>> {
    if !crate::perf::mc_has_glfw(mc_version) {
        return Err(anyhow!("the in-game HUD needs Minecraft 1.13 or newer"));
    }
    if !runtime_has_instrument(java) {
        return Err(anyhow!("this Java runtime cannot load agents (it has no java.instrument module)"));
    }
    let game = paths::game_dir(id);
    Ok(vec![
        format!("-javaagent:{}", agent_path()?.display()),
        format!("-Dvesper.hud.config={}", game.join("config").join("vesper-hud.json").display()),
        format!("-Dvesper.hud.options={}", game.join("options.txt").display()),
    ])
}

/// Turn the in-game HUD on or off for an instance. It works on every Java loader (vanilla, Fabric, Quilt,
/// Forge, NeoForge); turning it off also removes the optional Fabric add-on.
#[tauri::command]
pub fn hud_set(id: String, enabled: bool) -> CmdResult<()> {
    let mut inst = instances::load(&id)?;
    if enabled {
        if inst.loader == "rust" {
            // Horizon draws its own HUD: write the layout into its settings file
            let saved = get_hud();
            let hud = if saved.is_null() { crate::hud_rust::default_hud() } else { saved };
            crate::hud_rust::apply(&id, &hud)?;
        } else {
            if !crate::perf::mc_has_glfw(&inst.mc_version) {
                return Err(anyhow!("The in-game HUD needs Minecraft 1.13 or newer.").into());
            }
            sync(&id)?;
        }
    } else if inst.loader == "rust" {
        let mut off = get_hud();
        if off.is_null() { off = crate::hud_rust::default_hud(); }
        off["enabled"] = Value::Bool(false);
        crate::hud_rust::apply(&id, &off)?;
    } else {
        remove_old_jars(&paths::game_dir(&id).join("mods"));
    }
    inst.hud = enabled;
    instances::save(&inst)?;
    Ok(())
}

fn hud_file() -> std::path::PathBuf { paths::root().join("hud.json") }

/// The in-game editor saves into an instance's own copy of the settings. When one is newer than the launcher's
/// file, it holds changes made in game, so it becomes the launcher's copy (otherwise the next launch would undo them).
fn pull_newer() {
    let main = hud_file();
    let main_time = std::fs::metadata(&main).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = std::fs::read_dir(paths::instances()) {
        for e in rd.flatten() {
            let p = e.path().join("game").join("config").join("vesper-hud.json");
            if let Ok(t) = std::fs::metadata(&p).and_then(|m| m.modified()) {
                if t > main_time && best.as_ref().map_or(true, |(bt, _)| t > *bt) { best = Some((t, p)); }
            }
        }
    }
    if let Some((_, p)) = best {
        if let Ok(bytes) = std::fs::read(&p) {
            if bytes.len() <= MAX_CONFIG && serde_json::from_slice::<Value>(&bytes).map(|v| v.is_object()).unwrap_or(false) {
                let _ = std::fs::write(&main, bytes);
            }
        }
    }
}

/// The saved HUD settings, or `null` when nothing was saved yet (the UI then uses its defaults).
#[tauri::command]
pub fn get_hud() -> Value {
    pull_newer();
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
    crate::hud_rust::apply_all(&hud); // Rust instances get the same layout in their own settings file
    Ok(())
}

/// Copy the saved HUD settings into an instance's config folder (called before launching).
pub fn sync(id: &str) -> Result<()> {
    pull_newer();
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

/// Install the optional Fabric add-on (and Fabric API, which it needs). It adds the elements that need data from
/// inside the game: coordinates, ping, direction, armor and potion effects.
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

