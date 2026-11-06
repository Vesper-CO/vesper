//! Instances = isolated profiles. Each has its own game folder (mods, saves, options...),
//! version, loader, RAM and JVM arguments; the heavy game files are shared.
use crate::{error::CmdResult, launch::Running, paths};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};

const LOADERS: [&str; 5] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];
fn d_ram() -> u32 { 4096 }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    /// The resolved launch profile id (e.g. "fabric-loader-0.16.9-1.21.1"). Filled on first Play.
    #[serde(default)]
    pub version_id: String,
    #[serde(default = "d_ram")]
    pub ram_mb: u32,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub last_played: u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn load(id: &str) -> Result<Instance> {
    paths::safe_id(id)?;
    let b = std::fs::read(paths::instance_dir(id).join("instance.json")).with_context(|| format!("Instance '{id}' not found"))?;
    Ok(serde_json::from_slice(&b)?)
}

pub fn save(i: &Instance) -> Result<()> {
    paths::safe_id(&i.id)?;
    std::fs::create_dir_all(paths::game_dir(&i.id))?;
    std::fs::write(paths::instance_dir(&i.id).join("instance.json"), serde_json::to_vec_pretty(i)?)?;
    Ok(())
}

pub fn list() -> Result<Vec<Instance>> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(paths::instances()) {
        for e in rd.flatten() {
            if let Ok(i) = load(&e.file_name().to_string_lossy()) {
                out.push(i);
            }
        }
    }
    out.sort_by(|a, b| b.last_played.max(b.created).cmp(&a.last_played.max(a.created)));
    Ok(out)
}

pub fn create(name: &str, mc: &str, loader: &str, lv: Option<String>, ram: u32) -> Result<Instance> {
    if name.trim().is_empty() {
        bail!("Give the instance a name");
    }
    if !LOADERS.contains(&loader) {
        bail!("Unknown loader '{loader}'");
    }
    let slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let id = format!("{}-{}", slug.trim_matches('-').chars().take(24).collect::<String>(), &uuid::Uuid::new_v4().simple().to_string()[..6]);
    let i = Instance {
        id: id.trim_start_matches('-').to_string(),
        name: name.trim().into(),
        mc_version: mc.into(),
        loader: loader.into(),
        loader_version: lv.unwrap_or_default(),
        version_id: String::new(),
        ram_mb: ram.clamp(1024, 65536),
        jvm_args: String::new(),
        created: now(),
        last_played: 0,
    };
    save(&i)?;
    Ok(i)
}

#[tauri::command]
pub fn list_instances() -> CmdResult<Vec<Instance>> { Ok(list()?) }

#[tauri::command]
pub fn create_instance(name: String, mc_version: String, loader: String, loader_version: Option<String>, ram_mb: u32) -> CmdResult<Instance> {
    Ok(create(&name, &mc_version, &loader, loader_version.filter(|s| !s.is_empty()), ram_mb)?)
}

#[tauri::command]
pub fn update_instance(inst: Instance) -> CmdResult<Instance> {
    let old = load(&inst.id)?;
    let mut new = inst;
    new.created = old.created;
    new.last_played = old.last_played;
    new.ram_mb = new.ram_mb.clamp(1024, 65536);
    // Changing version or loader means the launch profile must be rebuilt on next Play.
    if new.mc_version != old.mc_version || new.loader != old.loader || new.loader_version != old.loader_version {
        new.version_id = String::new();
    } else {
        new.version_id = old.version_id;
    }
    save(&new)?;
    Ok(new)
}

#[tauri::command]
pub fn delete_instance(id: String, running: State<'_, Running>) -> CmdResult<()> {
    paths::safe_id(&id)?;
    if running.0.lock().unwrap().contains_key(&id) {
        return Err(anyhow!("Stop the game before deleting this instance").into());
    }
    std::fs::remove_dir_all(paths::instance_dir(&id)).context("Could not delete the instance folder")?;
    Ok(())
}

#[tauri::command]
pub fn open_instance_folder(app: AppHandle, id: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    paths::safe_id(&id)?;
    app.opener().open_path(paths::game_dir(&id).to_string_lossy().to_string(), None::<&str>)?;
    Ok(())
}
