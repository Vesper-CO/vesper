//! CurseForge (official API v1): search, project pages, one-click install with required dependencies,
//! modpack import and update-all. It plugs into the same per-instance `vesper-content.json` that
//! Modrinth uses, so installed content from both sites is listed, toggled and removed the same way.
//!
//! The API needs a personal key (free, console.curseforge.com). The user pastes it in Settings; a build
//! can also bake one in with the `CURSEFORGE_API_KEY` environment variable at compile time.
//! Results are reshaped here into the same JSON the Modrinth commands return, so the UI is shared.
use crate::{
    download::*,
    error::CmdResult,
    instances::{self, Instance},
    modrinth::{kind_dir, loaders_for, read_items, write_items, Item},
    paths, settings,
};
use anyhow::{anyhow, bail, Result};
use futures_util::future::BoxFuture;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
};
use tauri::AppHandle;

const API: &str = "https://api.curseforge.com/v1";
const GAME: u32 = 432; // Minecraft
const SOURCE: &str = "curseforge";

// ---------- plumbing ----------

fn api_key() -> Result<String> {
    let mut k = settings::load().curseforge_api_key.trim().to_string();
    if k.is_empty() {
        k = option_env!("CURSEFORGE_API_KEY").unwrap_or("").trim().to_string();
    }
    if k.is_empty() {
        bail!("CurseForge needs an API key. Get a free one at console.curseforge.com and paste it in Settings.");
    }
    Ok(k)
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value> {
    let r = req.header("x-api-key", api_key()?).send().await?;
    let status = r.status().as_u16();
    if status == 401 || status == 403 {
        bail!("CurseForge rejected the API key. Check it in Settings.");
    }
    Ok(r.error_for_status()?.json().await?)
}

async fn get(path: &str, q: &[(&str, String)]) -> Result<Value> {
    send(client().get(format!("{API}{path}")).query(q)).await
}

async fn post(path: &str, body: &Value) -> Result<Value> {
    send(client().post(format!("{API}{path}")).json(body)).await
}

fn class_id(kind: &str) -> Result<u32> {
    Ok(match kind {
        "mod" => 6,
        "resourcepack" => 12,
        "shader" => 6552,
        "modpack" => 4471,
        other => bail!("CurseForge has no '{other}' section"),
    })
}

fn kind_of(class: u64) -> Option<&'static str> {
    Some(match class {
        6 => "mod",
        12 => "resourcepack",
        6552 => "shader",
        4471 => "modpack",
        _ => return None,
    })
}

/// CurseForge's `modLoaderType` numbers for the loaders an instance can run.
fn loader_types(inst: &Instance) -> Vec<u32> {
    loaders_for(inst)
        .into_iter()
        .map(|l| match l {
            "forge" => 1,
            "fabric" => 4,
            "quilt" => 5,
            _ => 6, // neoforge
        })
        .collect()
}

fn names(v: &Value) -> Vec<&str> {
    v.as_array().into_iter().flatten().filter_map(|c| c["name"].as_str()).collect()
}

/// A search hit in the same shape as a Modrinth one.
fn hit(m: &Value) -> Value {
    json!({
        "project_id": m["id"].as_u64().unwrap_or(0).to_string(),
        "title": m["name"],
        "author": m["authors"][0]["name"],
        "description": m["summary"],
        "icon_url": m["logo"]["thumbnailUrl"],
        "downloads": m["downloadCount"],
        "display_categories": names(&m["categories"]),
    })
}

/// A file's Minecraft versions and mod loaders (CurseForge mixes both in one list).
fn split_versions(f: &Value) -> (Vec<String>, Vec<String>) {
    let (mut mc, mut loaders) = (vec![], vec![]);
    for g in f["gameVersions"].as_array().into_iter().flatten().filter_map(|g| g.as_str()) {
        if g.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            mc.push(g.to_string());
        } else {
            let l = g.to_ascii_lowercase();
            if ["fabric", "quilt", "forge", "neoforge"].contains(&l.as_str()) {
                loaders.push(l);
            }
        }
    }
    (mc, loaders)
}

fn release_name(f: &Value) -> &'static str {
    match f["releaseType"].as_u64() {
        Some(2) => "beta",
        Some(3) => "alpha",
        _ => "release",
    }
}

fn sha1_of(f: &Value) -> Option<String> {
    f["hashes"].as_array()?.iter().find(|h| h["algo"] == 1).and_then(|h| h["value"].as_str()).map(String::from)
}

/// The download link. Some authors switch off downloads by other apps: then there is no link, and we say so.
fn file_url(m: &Value, f: &Value) -> Result<String> {
    if let Some(u) = f["downloadUrl"].as_str().filter(|u| !u.is_empty()) {
        return Ok(u.to_string());
    }
    if m["allowModDistribution"] == false {
        bail!(
            "The author of {} does not allow other apps to download it. Download it from CurseForge and put it in the instance folder yourself.",
            m["name"].as_str().unwrap_or("this project")
        );
    }
    let (id, name) = (f["id"].as_u64().unwrap_or(0), f["fileName"].as_str().unwrap_or(""));
    Ok(format!("https://edge.forgecdn.net/files/{}/{}/{}", id / 1000, id % 1000, name.replace(' ', "%20").replace('+', "%2B")))
}

fn newest(mut files: Vec<Value>) -> Option<Value> {
    files.retain(|f| f["isAvailable"] != false);
    files.sort_by(|a, b| b["fileDate"].as_str().cmp(&a["fileDate"].as_str()));
    files.iter().find(|f| f["releaseType"] == 1).or(files.first()).cloned()
}

async fn files_for(mod_id: u64, mc: Option<&str>, loader: Option<u32>) -> Result<Vec<Value>> {
    let mut q: Vec<(&str, String)> = vec![("pageSize", "50".to_string())];
    if let Some(m) = mc.filter(|m| !m.is_empty()) {
        q.push(("gameVersion", m.into()));
    }
    if let Some(l) = loader {
        q.push(("modLoaderType", l.to_string()));
    }
    let r = get(&format!("/mods/{mod_id}/files"), &q).await?;
    Ok(r["data"].as_array().cloned().unwrap_or_default())
}

async fn pick_file(mod_id: u64, inst: &Instance, kind: &str) -> Result<Value> {
    let mut files = vec![];
    if kind == "mod" {
        let lts = loader_types(inst);
        if lts.is_empty() {
            files = files_for(mod_id, Some(&inst.mc_version), None).await?;
        }
        for lt in lts {
            files.extend(files_for(mod_id, Some(&inst.mc_version), Some(lt)).await?);
        }
    } else {
        files = files_for(mod_id, Some(&inst.mc_version), None).await?;
        // Horizon reads packs made for older versions too, so a Rust instance falls back to the newest.
        if files.is_empty() && inst.loader == "rust" {
            files = files_for(mod_id, None, None).await?;
        }
    }
    newest(files).ok_or_else(|| anyhow!("No version of this project works with {} {}", inst.loader, inst.mc_version))
}

// ---------- browsing ----------

#[tauri::command]
pub fn curseforge_available() -> bool {
    api_key().is_ok()
}

#[tauri::command]
pub async fn curseforge_categories(project_type: String) -> CmdResult<Vec<Value>> {
    let r = get("/categories", &[("gameId", GAME.to_string()), ("classId", class_id(&project_type)?.to_string())]).await?;
    let mut v: Vec<Value> = r["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["isClass"] != true)
        .map(|c| json!({ "id": c["id"].as_u64().unwrap_or(0).to_string(), "name": c["name"] }))
        .collect();
    v.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(v)
}

#[tauri::command]
pub async fn curseforge_search(query: String, project_type: String, category: Option<String>, loader: Option<String>, version: Option<String>, sort: String, offset: u32) -> CmdResult<Value> {
    let text = query.trim().to_string();
    let field = match sort.as_str() {
        "downloads" => 6,
        "follows" => 2,
        "newest" => 11,
        "updated" => 3,
        _ if text.is_empty() => 2,
        _ => 1,
    };
    let mut q: Vec<(&str, String)> = vec![
        ("gameId", GAME.to_string()),
        ("classId", class_id(&project_type)?.to_string()),
        ("pageSize", "20".into()),
        ("index", offset.to_string()),
        ("sortField", field.to_string()),
        ("sortOrder", "desc".into()),
    ];
    if !text.is_empty() {
        q.push(("searchFilter", text));
    }
    if let Some(c) = category.filter(|c| !c.is_empty()) {
        q.push(("categoryId", c));
    }
    if let Some(v) = version.filter(|v| !v.is_empty()) {
        q.push(("gameVersion", v));
    }
    if project_type == "mod" {
        let lt = match loader.as_deref() {
            Some("forge") => Some(1),
            Some("fabric") => Some(4),
            Some("quilt") => Some(5),
            Some("neoforge") => Some(6),
            _ => None,
        };
        if let Some(l) = lt {
            q.push(("modLoaderType", l.to_string()));
        }
    }
    let r = get("/mods/search", &q).await?;
    let hits: Vec<Value> = r["data"].as_array().into_iter().flatten().map(hit).collect();
    Ok(json!({ "hits": hits, "total_hits": r["pagination"]["totalCount"].as_u64().unwrap_or(0) }))
}

/// A project and its newest files, shaped like Modrinth's project and version objects.
#[tauri::command]
pub async fn curseforge_project(id: String) -> CmdResult<Value> {
    let id: u64 = id.parse()?;
    let (m, d, f) = futures_util::future::try_join3(
        get(&format!("/mods/{id}"), &[]),
        get(&format!("/mods/{id}/description"), &[]),
        get(&format!("/mods/{id}/files"), &[("pageSize", "50".to_string())]),
    )
    .await?;
    let m = &m["data"];
    let gallery: Vec<Value> = m["screenshots"].as_array().into_iter().flatten().map(|s| json!({ "url": s["url"], "title": s["title"] })).collect();
    let versions: Vec<Value> = f["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let (mc, loaders) = split_versions(v);
            json!({
                "id": v["id"].as_u64().unwrap_or(0).to_string(),
                "name": v["displayName"],
                "version_type": release_name(v),
                "version_number": v["fileName"],
                "game_versions": mc,
                "loaders": loaders,
                "changelog": "",
            })
        })
        .collect();
    Ok(json!({
        "project": {
            "title": m["name"],
            "description": m["summary"],
            "body": d["data"],
            "icon_url": m["logo"]["url"],
            "downloads": m["downloadCount"],
            "project_type": kind_of(m["classId"].as_u64().unwrap_or(0)).unwrap_or("mod"),
            "gallery": gallery,
        },
        "versions": versions,
    }))
}

// ---------- installing ----------

async fn place_file(app: &AppHandle, inst: &Instance, m: &Value, kind: &str, file: &Value) -> Result<()> {
    let title = m["name"].as_str().unwrap_or("Unknown").to_string();
    let pid = format!("cf:{}", m["id"].as_u64().unwrap_or(0));
    let filename = file["fileName"].as_str().ok_or_else(|| anyhow!("File has no name"))?.to_string();
    paths::safe_file_name(&filename)?;
    let dir = paths::game_dir(&inst.id).join(kind_dir(kind)?);
    let sha1 = sha1_of(file);
    let job = Job { url: file_url(m, file)?, path: dir.join(&filename), sha1: sha1.clone(), size: file["fileLength"].as_u64() };
    download_all(app, &format!("Installing {title}"), vec![job]).await?;

    let mut items = read_items(&inst.id);
    if let Some(pos) = items.iter().position(|i| i.project_id == pid) {
        let old = items.remove(pos);
        if old.filename != filename {
            let _ = std::fs::remove_file(dir.join(&old.filename));
            let _ = std::fs::remove_file(dir.join(format!("{}.disabled", old.filename)));
        }
    }
    items.push(Item {
        project_id: pid,
        version_id: file["id"].as_u64().unwrap_or(0).to_string(),
        title,
        kind: kind.into(),
        filename,
        sha1: sha1.unwrap_or_default(),
        icon: m["logo"]["thumbnailUrl"].as_str().unwrap_or("").into(),
        version_name: file["displayName"].as_str().unwrap_or("").into(),
        enabled: true,
        source: SOURCE.into(),
    });
    write_items(&inst.id, &items)
}

/// Install a project and, recursively, its required dependencies.
fn install_inner<'a>(app: &'a AppHandle, inst: &'a Instance, id: u64, file_id: Option<u64>, is_dep: bool, seen: &'a mut HashSet<u64>, out: &'a mut Vec<String>) -> BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        if !seen.insert(id) {
            return Ok(());
        }
        let m = get(&format!("/mods/{id}"), &[]).await?["data"].clone();
        let name = m["name"].as_str().unwrap_or("Unknown").to_string();
        // The same library may already be there from Modrinth (for example Fabric API): two copies would crash the game.
        if is_dep && read_items(&inst.id).iter().any(|i| i.title.eq_ignore_ascii_case(&name)) {
            return Ok(());
        }
        let kind = kind_of(m["classId"].as_u64().unwrap_or(0)).ok_or_else(|| anyhow!("{name} is not something Vesper can install into an instance"))?;
        if kind == "modpack" {
            bail!("Modpacks are installed as new instances");
        }
        if inst.loader == "rust" && kind == "mod" {
            bail!("Rust instances use Horizon's own mod library, not Java mods from CurseForge.");
        }
        if kind == "mod" && inst.loader == "vanilla" {
            bail!("Mods need a loader. Edit this instance and pick Fabric, Quilt, Forge or NeoForge.");
        }
        let file = match file_id {
            Some(f) => get(&format!("/mods/{id}/files/{f}"), &[]).await?["data"].clone(),
            None => pick_file(id, inst, kind).await?,
        };
        for d in file["dependencies"].as_array().into_iter().flatten() {
            if d["relationType"] != 3 {
                continue; // 3 = required
            }
            if let Some(dep) = d["modId"].as_u64() {
                if read_items(&inst.id).iter().any(|i| i.project_id == format!("cf:{dep}")) {
                    continue;
                }
                install_inner(app, inst, dep, None, true, seen, out).await?;
            }
        }
        place_file(app, inst, &m, kind, &file).await?;
        out.push(name);
        Ok(())
    })
}

#[tauri::command]
pub async fn curseforge_install(app: AppHandle, instance_id: String, project_id: String, file_id: Option<u64>) -> CmdResult<Vec<String>> {
    let inst = instances::load(&instance_id)?;
    let (mut seen, mut out) = (HashSet::new(), vec![]);
    install_inner(&app, &inst, project_id.parse()?, file_id, false, &mut seen, &mut out).await?;
    Ok(out)
}

/// Ask CurseForge for newer files of everything we installed from it.
#[tauri::command]
pub async fn curseforge_update_all(app: AppHandle, instance_id: String) -> CmdResult<usize> {
    let inst = instances::load(&instance_id)?;
    let items: Vec<Item> = read_items(&instance_id).into_iter().filter(|i| i.source == SOURCE).collect();
    if items.is_empty() {
        return Ok(0);
    }
    api_key()?;
    let mut updated = 0;
    for it in items {
        if inst.loader == "rust" && it.kind == "mod" {
            continue;
        }
        let Some(id) = it.project_id.strip_prefix("cf:").and_then(|s| s.parse::<u64>().ok()) else { continue };
        let Ok(m) = get(&format!("/mods/{id}"), &[]).await else { continue };
        let m = m["data"].clone();
        let Ok(file) = pick_file(id, &inst, &it.kind).await else { continue };
        if file["id"].as_u64().map(|x| x.to_string()).as_deref() != Some(it.version_id.as_str()) && place_file(&app, &inst, &m, &it.kind, &file).await.is_ok() {
            updated += 1;
        }
    }
    Ok(updated)
}

// ---------- modpacks ----------

fn folder_for(class: u64) -> Option<&'static str> {
    match class {
        12 => Some("resourcepacks"),
        6552 => Some("shaderpacks"),
        17 => None, // worlds are not copied into a new instance
        _ => Some("mods"),
    }
}

/// Import a CurseForge modpack (manifest.json + overrides) as a new instance. Files whose authors forbid
/// downloads by other apps cannot be fetched: they are listed in `vesper-missing-mods.txt` in the game folder.
#[tauri::command]
pub async fn curseforge_install_modpack(app: AppHandle, project_id: String, file_id: Option<u64>) -> CmdResult<Value> {
    let id: u64 = project_id.parse()?;
    let m = get(&format!("/mods/{id}"), &[]).await?["data"].clone();
    let pack = match file_id {
        Some(f) => get(&format!("/mods/{id}/files/{f}"), &[]).await?["data"].clone(),
        None => newest(files_for(id, None, None).await?).ok_or_else(|| anyhow!("This modpack has no versions"))?,
    };
    let tmp = paths::tmp().join(format!("{}.zip", uuid::Uuid::new_v4().simple()));
    download_all(&app, "Downloading modpack", vec![Job { url: file_url(&m, &pack)?, path: tmp.clone(), sha1: sha1_of(&pack), size: None }]).await?;
    let r = import_pack(&app, &tmp).await;
    let _ = std::fs::remove_file(&tmp);
    let (inst, skipped) = r?;
    Ok(json!({ "instance": inst, "skipped": skipped }))
}

async fn import_pack(app: &AppHandle, zip_path: &std::path::Path) -> Result<(Instance, Vec<String>)> {
    let p = zip_path.to_owned();
    let manifest: Value = tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(p)?)?;
        let mut s = String::new();
        z.by_name("manifest.json").map_err(|_| anyhow!("Not a CurseForge modpack (manifest.json missing)"))?.read_to_string(&mut s)?;
        Ok(serde_json::from_str(&s)?)
    })
    .await??;

    let mc = manifest["minecraft"]["version"].as_str().ok_or_else(|| anyhow!("Modpack does not say which Minecraft version it needs"))?;
    let loaders = manifest["minecraft"]["modLoaders"].as_array().cloned().unwrap_or_default();
    let primary = loaders.iter().find(|l| l["primary"] == true).or(loaders.first()).and_then(|l| l["id"].as_str()).unwrap_or("");
    let (loader, lv) = match primary.split_once('-') {
        Some((l @ ("forge" | "fabric" | "quilt" | "neoforge"), v)) => (l, Some(v.to_string())),
        _ => ("vanilla", None),
    };
    let name = manifest["name"].as_str().unwrap_or("Imported modpack");
    let inst = instances::create(name, mc, loader, lv, 4096)?;
    let game = paths::game_dir(&inst.id);

    let wanted: Vec<u64> = manifest["files"].as_array().into_iter().flatten().filter(|f| f["required"] != false).filter_map(|f| f["fileID"].as_u64()).collect();
    let mut files: Vec<Value> = vec![];
    for chunk in wanted.chunks(100) {
        let r = post("/mods/files", &json!({ "fileIds": chunk })).await?;
        files.extend(r["data"].as_array().cloned().unwrap_or_default());
    }
    let mod_ids: Vec<u64> = files.iter().filter_map(|f| f["modId"].as_u64()).collect::<HashSet<_>>().into_iter().collect();
    let mut mods: HashMap<u64, Value> = HashMap::new();
    for chunk in mod_ids.chunks(100) {
        let r = post("/mods", &json!({ "modIds": chunk })).await?;
        for x in r["data"].as_array().into_iter().flatten() {
            if let Some(i) = x["id"].as_u64() {
                mods.insert(i, x.clone());
            }
        }
    }

    let (mut jobs, mut skipped) = (vec![], vec![]);
    for f in &files {
        let proj = mods.get(&f["modId"].as_u64().unwrap_or(0)).cloned().unwrap_or(Value::Null);
        let Some(folder) = folder_for(proj["classId"].as_u64().unwrap_or(6)) else { continue };
        let fname = f["fileName"].as_str().unwrap_or("");
        if paths::safe_file_name(fname).is_err() {
            continue;
        }
        match file_url(&proj, f) {
            Ok(url) => jobs.push(Job { url, path: game.join(folder).join(fname), sha1: sha1_of(f), size: f["fileLength"].as_u64() }),
            Err(_) => skipped.push(format!("{} ({}): {}/files/{}", proj["name"].as_str().unwrap_or("Unknown"), fname, proj["links"]["websiteUrl"].as_str().unwrap_or("https://www.curseforge.com/minecraft"), f["id"].as_u64().unwrap_or(0))),
        }
    }
    download_all(app, "Modpack files", jobs).await?;
    if !skipped.is_empty() {
        let note = format!("These files could not be downloaded by Vesper because their authors do not allow other apps to download them.\nDownload each one from its page and put it in the matching folder (mods, resourcepacks or shaderpacks).\n\n{}\n", skipped.join("\n"));
        std::fs::create_dir_all(&game)?;
        std::fs::write(game.join("vesper-missing-mods.txt"), note)?;
    }

    // The overrides folder (configs, options, ...) is copied over the game folder.
    let overrides = manifest["overrides"].as_str().unwrap_or("overrides").to_string();
    let (zp, game2) = (zip_path.to_owned(), game.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(zp)?)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let Some(path) = e.enclosed_name() else { continue };
            let rel = match path.strip_prefix(&overrides) {
                Ok(r) if !r.as_os_str().is_empty() => r.to_path_buf(),
                _ => continue,
            };
            let out = game2.join(rel);
            if e.is_dir() {
                std::fs::create_dir_all(&out)?;
                continue;
            }
            if let Some(par) = out.parent() {
                std::fs::create_dir_all(par)?;
            }
            std::io::copy(&mut e, &mut std::fs::File::create(out)?)?;
        }
        Ok(())
    })
    .await??;
    Ok((inst, skipped))
}
