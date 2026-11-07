//! Modrinth (official public API v2): search, project pages, versions, one-click install with
//! dependency resolution, enable/disable, update-all. Installed content is tracked per instance
//! in `vesper-content.json` so we can update it later.
use crate::{download::*, error::CmdResult, instances::{self, Instance}, mrpack, paths};
use anyhow::{anyhow, bail, Result};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::PathBuf};
use tauri::AppHandle;

const API: &str = "https://api.modrinth.com/v2";

fn kind_dir(kind: &str) -> Result<&'static str> {
    Ok(match kind {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        other => bail!("'{other}' cannot be installed into an instance"),
    })
}

/// Loaders whose mods run on this instance (Quilt can load Fabric mods).
fn loaders_for(inst: &Instance) -> Vec<&'static str> {
    match inst.loader.as_str() {
        "fabric" => vec!["fabric"],
        "quilt" => vec!["quilt", "fabric"],
        "forge" => vec!["forge"],
        "neoforge" => vec!["neoforge"],
        _ => vec![],
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Item {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub kind: String,
    pub filename: String,
    pub sha1: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub version_name: String,
    /// Computed when listing, never trusted from disk.
    #[serde(default)]
    pub enabled: bool,
}

fn manifest_path(id: &str) -> PathBuf { paths::instance_dir(id).join("vesper-content.json") }
fn read_items(id: &str) -> Vec<Item> {
    std::fs::read(manifest_path(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn write_items(id: &str, items: &[Item]) -> Result<()> {
    std::fs::write(manifest_path(id), serde_json::to_vec_pretty(items)?)?;
    Ok(())
}

// ---------- browsing ----------

#[tauri::command]
pub async fn modrinth_categories() -> CmdResult<Value> {
    Ok(get_json(&format!("{API}/tag/category")).await?)
}

#[tauri::command]
pub async fn modrinth_search(query: String, project_type: String, categories: Vec<String>, loader: Option<String>, version: Option<String>, sort: String, offset: u32) -> CmdResult<Value> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{project_type}")]];
    if project_type == "mod" || project_type == "modpack" {
        if let Some(l) = loader.filter(|l| !l.is_empty()) {
            facets.push(vec![format!("categories:{l}")]);
        }
    }
    if let Some(v) = version.filter(|v| !v.is_empty()) {
        facets.push(vec![format!("versions:{v}")]);
    }
    for c in categories {
        facets.push(vec![format!("categories:{c}")]);
    }
    let q: Vec<(&str, String)> = vec![("query", query), ("facets", serde_json::to_string(&facets)?), ("limit", "20".into()), ("offset", offset.to_string()), ("index", sort)];
    let r = client().get(format!("{API}/search")).query(&q).send().await?.error_for_status()?;
    Ok(r.json().await?)
}

#[tauri::command]
pub async fn modrinth_project(id: String) -> CmdResult<Value> {
    Ok(get_json(&format!("{API}/project/{id}")).await?)
}

async fn list_versions(pid: &str, loaders: Vec<&str>, mc: Option<&str>) -> Result<Vec<Value>> {
    let mut q: Vec<(&str, String)> = vec![];
    if !loaders.is_empty() {
        q.push(("loaders", serde_json::to_string(&loaders)?));
    }
    if let Some(m) = mc.filter(|m| !m.is_empty()) {
        q.push(("game_versions", serde_json::to_string(&[m])?));
    }
    let r = client().get(format!("{API}/project/{pid}/version")).query(&q).send().await?.error_for_status()?;
    Ok(r.json().await?)
}

#[tauri::command]
pub async fn modrinth_versions(id: String) -> CmdResult<Vec<Value>> {
    Ok(list_versions(&id, vec![], None).await?)
}

// ---------- installing ----------

async fn pick_version(pid: &str, inst: &Instance, kind: &str) -> Result<Value> {
    let loaders = if kind == "mod" { loaders_for(inst) } else { vec![] };
    let vs = list_versions(pid, loaders, Some(&inst.mc_version)).await?;
    vs.iter().find(|v| v["version_type"] == "release").or(vs.first()).cloned().ok_or_else(|| anyhow!("No version of this project works with {} {}", inst.loader, inst.mc_version))
}

#[allow(clippy::too_many_arguments)]
async fn place_version(app: &AppHandle, inst: &Instance, pid: &str, title: &str, icon: &str, kind: &str, ver: &Value) -> Result<()> {
    let files = ver["files"].as_array().ok_or_else(|| anyhow!("Version has no files"))?;
    let f = files.iter().find(|f| f["primary"] == true).or(files.first()).ok_or_else(|| anyhow!("Version has no files"))?;
    let filename = f["filename"].as_str().ok_or_else(|| anyhow!("File has no name"))?.to_string();
    paths::safe_file_name(&filename)?;
    let dir = paths::game_dir(&inst.id).join(kind_dir(kind)?);
    let sha1 = f["hashes"]["sha1"].as_str().unwrap_or("").to_string();
    let job = Job { url: f["url"].as_str().ok_or_else(|| anyhow!("File has no url"))?.into(), path: dir.join(&filename), sha1: (!sha1.is_empty()).then(|| sha1.clone()), size: f["size"].as_u64() };
    download_all(app, &format!("Installing {title}"), vec![job]).await?;

    let mut items = read_items(&inst.id);
    if let Some(pos) = items.iter().position(|i| i.project_id == pid) {
        let old = items.remove(pos);
        if old.filename != filename {
            let _ = std::fs::remove_file(dir.join(&old.filename));
            let _ = std::fs::remove_file(dir.join(format!("{}.disabled", old.filename)));
        }
    }
    items.push(Item { project_id: pid.into(), version_id: ver["id"].as_str().unwrap_or("").into(), title: title.into(), kind: kind.into(), filename, sha1, icon: icon.into(), version_name: ver["version_number"].as_str().unwrap_or("").into(), enabled: true });
    write_items(&inst.id, &items)
}

/// Install a project and, recursively, its required dependencies.
fn install_inner<'a>(app: &'a AppHandle, inst: &'a Instance, pid: String, version_id: Option<String>, seen: &'a mut HashSet<String>, out: &'a mut Vec<String>) -> BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        if !seen.insert(pid.clone()) {
            return Ok(());
        }
        let proj: Value = get_json(&format!("{API}/project/{pid}")).await?;
        let kind = proj["project_type"].as_str().unwrap_or("mod").to_string();
        if kind == "modpack" {
            bail!("Modpacks are installed as new instances");
        }
        if inst.loader == "rust" {
            bail!("Mods are not available for Rust instances yet.");
        }
        if kind == "mod" && inst.loader == "vanilla" {
            bail!("Mods need a loader. Edit this instance and pick Fabric, Quilt, Forge or NeoForge.");
        }
        let ver: Value = match &version_id {
            Some(v) => get_json(&format!("{API}/version/{v}")).await?,
            None => pick_version(&pid, inst, &kind).await?,
        };
        for d in ver["dependencies"].as_array().into_iter().flatten() {
            if d["dependency_type"] != "required" {
                continue;
            }
            if let Some(dep) = d["project_id"].as_str() {
                if read_items(&inst.id).iter().any(|i| i.project_id == dep) {
                    continue;
                }
                install_inner(app, inst, dep.to_string(), d["version_id"].as_str().map(String::from), seen, out).await?;
            }
        }
        let title = proj["title"].as_str().unwrap_or("Unknown").to_string();
        place_version(app, inst, &pid, &title, proj["icon_url"].as_str().unwrap_or(""), &kind, &ver).await?;
        out.push(title);
        Ok(())
    })
}

#[tauri::command]
pub async fn modrinth_install(app: AppHandle, instance_id: String, project_id: String, version_id: Option<String>) -> CmdResult<Vec<String>> {
    let inst = instances::load(&instance_id)?;
    let (mut seen, mut out) = (HashSet::new(), vec![]);
    install_inner(&app, &inst, project_id, version_id, &mut seen, &mut out).await?;
    Ok(out)
}

#[tauri::command]
pub async fn modrinth_install_modpack(app: AppHandle, project_id: String, version_id: Option<String>) -> CmdResult<Instance> {
    let ver: Value = match version_id {
        Some(v) => get_json(&format!("{API}/version/{v}")).await?,
        None => {
            let vs = list_versions(&project_id, vec![], None).await?;
            vs.iter().find(|v| v["version_type"] == "release").or(vs.first()).cloned().ok_or_else(|| anyhow!("This modpack has no versions"))?
        }
    };
    let files = ver["files"].as_array().ok_or_else(|| anyhow!("No files"))?;
    let f = files.iter().find(|f| f["primary"] == true).or(files.first()).ok_or_else(|| anyhow!("No files"))?;
    let tmp = paths::tmp().join(format!("{}.mrpack", uuid::Uuid::new_v4().simple()));
    download_all(&app, "Downloading modpack", vec![Job { url: f["url"].as_str().unwrap_or_default().into(), path: tmp.clone(), sha1: f["hashes"]["sha1"].as_str().map(String::from), size: None }]).await?;
    let r = mrpack::import_path(&app, &tmp).await;
    let _ = std::fs::remove_file(&tmp);
    Ok(r?)
}

#[tauri::command]
pub async fn import_mrpack(app: AppHandle, path: String) -> CmdResult<Instance> {
    Ok(mrpack::import_path(&app, std::path::Path::new(&path)).await?)
}

// ---------- managing installed content ----------

#[tauri::command]
pub fn modrinth_installed(instance_id: String) -> CmdResult<Vec<Item>> {
    paths::safe_id(&instance_id)?;
    let game = paths::game_dir(&instance_id);
    let mut items = read_items(&instance_id);
    for i in items.iter_mut() {
        let dir = game.join(kind_dir(&i.kind)?);
        i.enabled = dir.join(&i.filename).exists();
    }
    items.retain(|i| {
        let dir = game.join(kind_dir(&i.kind).unwrap_or("mods"));
        dir.join(&i.filename).exists() || dir.join(format!("{}.disabled", i.filename)).exists()
    });
    // Files dropped in by hand or by a modpack are listed too, so they can be toggled.
    let known: HashSet<String> = items.iter().map(|i| i.filename.clone()).collect();
    for (kind, folder, ext) in [("mod", "mods", ".jar"), ("resourcepack", "resourcepacks", ".zip"), ("shader", "shaderpacks", ".zip")] {
        if let Ok(rd) = std::fs::read_dir(game.join(folder)) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let base = name.strip_suffix(".disabled").unwrap_or(&name).to_string();
                if known.contains(&base) || !base.ends_with(ext) {
                    continue;
                }
                items.push(Item { project_id: String::new(), version_id: String::new(), title: base.clone(), kind: kind.into(), filename: base, sha1: String::new(), icon: String::new(), version_name: String::new(), enabled: !name.ends_with(".disabled") });
            }
        }
    }
    Ok(items)
}

#[tauri::command]
pub fn modrinth_toggle(instance_id: String, kind: String, filename: String) -> CmdResult<bool> {
    paths::safe_id(&instance_id)?;
    paths::safe_file_name(&filename)?;
    let dir = paths::game_dir(&instance_id).join(kind_dir(&kind)?);
    let on = dir.join(&filename);
    let off = dir.join(format!("{filename}.disabled"));
    if on.exists() {
        std::fs::rename(&on, &off)?;
        Ok(false)
    } else if off.exists() {
        std::fs::rename(&off, &on)?;
        Ok(true)
    } else {
        Err(anyhow!("File not found").into())
    }
}

#[tauri::command]
pub fn modrinth_remove(instance_id: String, kind: String, filename: String) -> CmdResult<()> {
    paths::safe_id(&instance_id)?;
    paths::safe_file_name(&filename)?;
    let dir = paths::game_dir(&instance_id).join(kind_dir(&kind)?);
    let _ = std::fs::remove_file(dir.join(&filename));
    let _ = std::fs::remove_file(dir.join(format!("{filename}.disabled")));
    let mut items = read_items(&instance_id);
    items.retain(|i| !(i.filename == filename && i.kind == kind));
    Ok(write_items(&instance_id, &items)?)
}

/// Ask Modrinth for newer versions of everything we installed (matched by file hash).
#[tauri::command]
pub async fn modrinth_update_all(app: AppHandle, instance_id: String) -> CmdResult<usize> {
    let inst = instances::load(&instance_id)?;
    if inst.loader == "rust" {
        return Err(anyhow!("Mods are not available for Rust instances yet.").into());
    }
    let items: Vec<Item> = read_items(&instance_id).into_iter().filter(|i| !i.sha1.is_empty()).collect();
    let mut updated = 0;
    for kind in ["mod", "resourcepack", "shader"] {
        let group: Vec<&Item> = items.iter().filter(|i| i.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        let hashes: Vec<&str> = group.iter().map(|i| i.sha1.as_str()).collect();
        let mut body = json!({"hashes": hashes, "algorithm": "sha1", "game_versions": [inst.mc_version]});
        let loaders = loaders_for(&inst);
        if kind == "mod" && !loaders.is_empty() {
            body["loaders"] = json!(loaders);
        }
        let res: Value = client().post(format!("{API}/version_files/update")).json(&body).send().await?.error_for_status()?.json().await?;
        for it in group {
            if let Some(nv) = res.get(&it.sha1) {
                if nv["id"].as_str() != Some(it.version_id.as_str()) {
                    place_version(&app, &inst, &it.project_id, &it.title, &it.icon, &it.kind, nv).await?;
                    updated += 1;
                }
            }
        }
    }
    Ok(updated)
}
