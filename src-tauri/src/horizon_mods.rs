//! Horizon's own mod library (playhorizon.dev/mods) for "Rust" instances. It is the only mod source
//! Rust instances use: Java mods from Modrinth do not run in Horizon.
//!
//! The library is one file, `index.json`, built by github.com/horizon-rust/horizon-mods. Every download
//! is checked against it before it reaches the game folder: HTTPS from GitHub's release servers, the
//! size and SHA-256 the index lists, a plain file name, and the index's list of files taken down.
//!
//! Content mods (`.hmod` files with no code) are installed here: the game reads them from
//! `horizon-mods/` when it starts. Native mods (`.hnmod`, 100+ of the library) are Rust source that is
//! compiled into the game itself: Vesper's client is built with the ones listed in the `vesper-rust`
//! repository's `native-mods.txt` (they show as "Built in", and are switched on and off in the game's
//! Options > Mods). The others are listed but cannot be installed one by one.
use crate::{download::client, error::CmdResult, instances, paths};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf, sync::{Arc, Mutex}, time::{Duration, Instant}};

const INDEX_URL: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
const SITE: &str = "https://playhorizon.dev/mods";
const MAX_INDEX: usize = 16 << 20;
const MAX_FILE: u64 = 64 << 20;
const FRESH: Duration = Duration::from_secs(600);
const PAGE: usize = 20;

// ---------- the index ----------

#[derive(Deserialize, Clone, Default)]
struct Index {
    #[serde(default)]
    schema: u32,
    #[serde(default)]
    projects: Vec<Value>,
    #[serde(default)]
    revoked: Vec<Revoked>,
}

#[derive(Deserialize, Clone)]
struct Revoked {
    id: String,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Project {
    id: String,
    kind: String,
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    authors: Vec<String>,
    #[serde(default)]
    license: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    versions: Vec<Version>,
}

#[derive(Deserialize, Clone)]
struct Version {
    version: String,
    #[serde(default)]
    game: String,
    #[serde(default)]
    capabilities: Vec<String>,
    file: File,
}

#[derive(Deserialize, Clone)]
struct File {
    name: String,
    url: String,
    size: u64,
    sha256: String,
}

struct Loaded {
    at: Instant,
    projects: Arc<Vec<Project>>,
    revoked: Arc<Vec<Revoked>>,
}
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);

fn cache_path() -> PathBuf { paths::rust().join("horizon-index.json") }

fn parse(text: &str) -> Result<(Vec<Project>, Vec<Revoked>)> {
    let idx: Index = serde_json::from_str(text).context("The Horizon mod library is not valid JSON")?;
    if idx.schema != 1 {
        bail!("The Horizon mod library uses a newer format (schema {}). Update Vesper.", idx.schema);
    }
    // A project this build cannot read is skipped, never fatal for the rest.
    let projects = idx.projects.into_iter().filter_map(|v| serde_json::from_value::<Project>(v).ok()).collect();
    Ok((projects, idx.revoked))
}

fn fresh() -> Option<(Arc<Vec<Project>>, Arc<Vec<Revoked>>)> {
    let g = LOADED.lock().unwrap();
    g.as_ref().filter(|l| l.at.elapsed() < FRESH).map(|l| (l.projects.clone(), l.revoked.clone()))
}

async fn fetch_text() -> Result<String> {
    let r = client().get(INDEX_URL).send().await.context("Could not reach the Horizon mod library")?;
    let r = r.error_for_status().context("The Horizon mod library did not answer")?;
    let b = r.bytes().await?;
    if b.len() > MAX_INDEX {
        bail!("The Horizon mod library is larger than expected");
    }
    Ok(String::from_utf8(b.to_vec())?)
}

/// The library: the copy fetched in the last ten minutes, else a fresh one (kept on disk), else the
/// copy kept last time, so browsing still works offline.
async fn library() -> Result<(Arc<Vec<Project>>, Arc<Vec<Revoked>>)> {
    if let Some(l) = fresh() {
        return Ok(l);
    }
    let fetched = match fetch_text().await {
        Ok(text) => parse(&text).map(|p| (p, text)),
        Err(e) => Err(e),
    };
    let (projects, revoked) = match fetched {
        Ok(((p, r), text)) => {
            let _ = std::fs::create_dir_all(paths::rust());
            let _ = std::fs::write(cache_path(), text);
            (p, r)
        }
        Err(e) => match std::fs::read_to_string(cache_path()).ok().and_then(|k| parse(&k).ok()) {
            Some(kept) => kept,
            None => return Err(e),
        },
    };
    let (projects, revoked) = (Arc::new(projects), Arc::new(revoked));
    *LOADED.lock().unwrap() = Some(Loaded { at: Instant::now(), projects: projects.clone(), revoked: revoked.clone() });
    Ok((projects, revoked))
}

fn is_revoked(revoked: &[Revoked], id: &str, sha: &str) -> bool {
    revoked.iter().any(|r| r.id == id && r.sha256.as_ref().map_or(true, |h| h.eq_ignore_ascii_case(sha)))
}

/// The newest version made for this Minecraft version that was not taken down.
fn pick<'a>(p: &'a Project, revoked: &[Revoked], game: &str) -> Option<&'a Version> {
    p.versions.iter().find(|v| v.game == game && !is_revoked(revoked, &p.id, &v.file.sha256))
}

/// A `.hmod` with code is not run by the game any more; only content (capability `registries`) is.
fn has_code(v: &Version) -> bool {
    v.file.name.ends_with(".hmod") && v.capabilities.iter().any(|c| c != "registries")
}

/// "builtin" (a native mod compiled into the installed client), "ok" (installable), "native" (a native
/// mod that is not in the client), "code" (a code mod the game no longer runs),
/// "version" (nothing for this Minecraft version), "other" (not a mod: modpacks, worlds...).
fn support(p: &Project, revoked: &[Revoked], game: &str) -> &'static str {
    if p.kind == "native-mod" {
        return if crate::rust::builtin_ids().contains(&p.id) { "builtin" } else { "native" };
    }
    if p.kind != "mod" {
        return "other";
    }
    match pick(p, revoked, game) {
        None => "version",
        Some(v) if has_code(v) => "code",
        Some(_) => "ok",
    }
}

// ---------- what is installed in an instance ----------

#[derive(Serialize, Deserialize, Clone)]
struct Item {
    id: String,
    title: String,
    version: String,
    filename: String,
    sha256: String,
    #[serde(default)]
    icon: String,
}

fn manifest_path(inst: &str) -> PathBuf { paths::instance_dir(inst).join("vesper-horizon.json") }
fn read_items(inst: &str) -> Vec<Item> {
    std::fs::read(manifest_path(inst)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn write_items(inst: &str, items: &[Item]) -> Result<()> {
    std::fs::write(manifest_path(inst), serde_json::to_vec_pretty(items)?)?;
    Ok(())
}
fn mods_dir(inst: &str) -> PathBuf { paths::game_dir(inst).join("horizon-mods") }

// The game keeps which content mods are on in `horizon-mods.txt`, one `mod.<id>:true` line each
// (docs/mods.md). A mod that has no line is off.
fn settings_path(inst: &str) -> PathBuf { paths::game_dir(inst).join("horizon-mods.txt") }
fn key(id: &str) -> String { format!("mod.{id}:") }
fn is_enabled(inst: &str, id: &str) -> bool {
    let k = format!("{}true", key(id));
    std::fs::read_to_string(settings_path(inst)).map(|t| t.lines().any(|l| l.trim() == k)).unwrap_or(false)
}
fn set_enabled(inst: &str, id: &str, on: bool) -> Result<()> {
    let k = key(id);
    let text = std::fs::read_to_string(settings_path(inst)).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().filter(|l| !l.trim_start().starts_with(&k)).map(String::from).collect();
    if on {
        lines.push(format!("{k}true"));
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    std::fs::create_dir_all(paths::game_dir(inst))?;
    std::fs::write(settings_path(inst), out)?;
    Ok(())
}

// ---------- built-in native mods start off ----------

/// `text` (a `horizon-mods.txt`) with a `mod.<id>:false` line added for each id that has no line yet.
fn with_off_lines(text: &str, ids: &[String]) -> String {
    let mut out = text.to_string();
    for id in ids {
        let k = key(id);
        if !out.lines().any(|l| l.trim_start().starts_with(&k)) {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{k}false\n"));
        }
    }
    out
}

fn seen_path(inst: &str) -> PathBuf { paths::instance_dir(inst).join("vesper-native-seen.json") }
fn read_seen(inst: &str) -> Vec<String> {
    std::fs::read(seen_path(inst)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn write_seen(inst: &str, seen: &[String]) -> Result<()> {
    std::fs::write(seen_path(inst), serde_json::to_vec(seen)?)?;
    Ok(())
}

/// Native mods compiled into the game are on in the game until switched off. Vesper starts them off:
/// the first time an instance meets a built-in mod it writes `mod.<id>:false` for it, once, and
/// remembers it did. After that the choice is the player's, made here or in the game under
/// Options > Mods. A mod the player already has a line for (on or off) is left as it is.
pub fn start_builtin_off(inst: &str) -> Result<()> {
    let ids = crate::rust::builtin_ids();
    if ids.is_empty() {
        return Ok(());
    }
    let mut seen = read_seen(inst);
    let fresh: Vec<String> = ids.into_iter().filter(|i| !seen.contains(i)).collect();
    if fresh.is_empty() {
        return Ok(());
    }
    let text = std::fs::read_to_string(settings_path(inst)).unwrap_or_default();
    let out = with_off_lines(&text, &fresh);
    if out != text {
        std::fs::create_dir_all(paths::game_dir(inst))?;
        std::fs::write(settings_path(inst), out)?;
    }
    seen.extend(fresh);
    write_seen(inst, &seen)
}

/// Whether a built-in mod is on for the instance: its line says so, and with no line yet it is off
/// until the first launch writes one (the game itself would treat a missing line as on).
fn native_on(inst: &str, id: &str) -> bool {
    let text = std::fs::read_to_string(settings_path(inst)).unwrap_or_default();
    let k = key(id);
    match text.lines().map(str::trim).find_map(|l| l.strip_prefix(k.as_str())) {
        Some(v) => v.trim() == "true",
        None => read_seen(inst).iter().any(|s| s == id),
    }
}

/// Writes `mod.<id>:true` or `:false` and keeps every other line (the game's key choices, content mods).
fn set_native(inst: &str, id: &str, on: bool) -> Result<()> {
    let k = key(id);
    let text = std::fs::read_to_string(settings_path(inst)).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().filter(|l| !l.trim_start().starts_with(&k)).map(String::from).collect();
    lines.push(format!("{k}{on}"));
    std::fs::create_dir_all(paths::game_dir(inst))?;
    std::fs::write(settings_path(inst), lines.join("\n") + "\n")?;
    // The player has decided: the first-launch step must not touch it again.
    let mut seen = read_seen(inst);
    if !seen.iter().any(|s| s == id) {
        seen.push(id.to_string());
        write_seen(inst, &seen)?;
    }
    Ok(())
}

// ---------- installing ----------

/// Files come from GitHub's release servers, as the index promises (docs/api/mod-index.md).
fn check_url(url: &str) -> Result<()> {
    let ok = url.starts_with("https://github.com/") && url.contains("/releases/download/") && !url.contains("..") && !url.contains('?') && !url.contains('#');
    if !ok {
        bail!("The library lists a download that is not on GitHub's release servers, so it was not installed.");
    }
    Ok(())
}

async fn download_checked(f: &File) -> Result<Vec<u8>> {
    check_url(&f.url)?;
    paths::safe_file_name(&f.name)?;
    if !f.name.ends_with(".hmod") {
        bail!("Unexpected file type: {}", f.name);
    }
    if f.size == 0 || f.size > MAX_FILE || f.sha256.len() != 64 || !f.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("The library lists {} without a usable size and checksum, so it was not installed.", f.name);
    }
    let r = client().get(&f.url).send().await.with_context(|| format!("Could not download {}", f.name))?;
    let r = r.error_for_status().with_context(|| format!("Could not download {}", f.name))?;
    if r.content_length().is_some_and(|n| n > MAX_FILE) {
        bail!("{} is larger than the library says", f.name);
    }
    let bytes = r.bytes().await?.to_vec();
    if bytes.len() as u64 != f.size {
        bail!("{} is not the size the library lists, so it was not installed.", f.name);
    }
    if hex::encode(Sha256::digest(&bytes)) != f.sha256.to_ascii_lowercase() {
        bail!("{} did not match its checksum, so it was not installed.", f.name);
    }
    Ok(bytes)
}

async fn install_one(inst: &str, game: &str, p: &Project, revoked: &[Revoked]) -> Result<()> {
    match support(p, revoked, game) {
        "ok" => {}
        "builtin" => bail!("{} is already built into the game.", p.name),
        "native" => bail!("{} is a native mod: it has to be compiled into the game, and is not in this build of it.", p.name),
        "code" => bail!("{} contains code that the game no longer runs.", p.name),
        "other" => bail!("{} is not a mod.", p.name),
        _ => bail!("{} has no version for Minecraft {game}.", p.name),
    }
    let v = pick(p, revoked, game).ok_or_else(|| anyhow!("{} has no version for Minecraft {game}.", p.name))?;
    let bytes = download_checked(&v.file).await?;

    let dir = mods_dir(inst);
    std::fs::create_dir_all(&dir)?;
    let part = dir.join(format!(".{}.part", v.file.name));
    std::fs::write(&part, &bytes)?;
    std::fs::rename(&part, dir.join(&v.file.name))?;

    let mut items = read_items(inst);
    if let Some(pos) = items.iter().position(|i| i.id == p.id) {
        let old = items.remove(pos);
        if old.filename != v.file.name {
            let _ = std::fs::remove_file(dir.join(&old.filename));
        }
    }
    items.push(Item { id: p.id.clone(), title: p.name.clone(), version: v.version.clone(), filename: v.file.name.clone(), sha256: v.file.sha256.to_ascii_lowercase(), icon: p.icon.clone().unwrap_or_default() });
    write_items(inst, &items)?;
    // Installed means on: the game's Options > Mods still has the switch.
    set_enabled(inst, &p.id, true)
}

fn load_rust_instance(id: &str) -> Result<instances::Instance> {
    let inst = instances::load(id)?;
    if inst.loader != "rust" {
        bail!("The Horizon mod library is only for Rust instances.");
    }
    Ok(inst)
}

// ---------- commands ----------

fn hit(p: &Project, support: &str, installed: Option<&Item>, game: &str, revoked: &[Revoked]) -> Value {
    let newest = pick(p, revoked, game).map(|v| v.version.clone()).unwrap_or_default();
    json!({
        "id": p.id, "kind": p.kind, "name": p.name, "summary": p.summary,
        "authors": p.authors, "icon": p.icon, "downloads": p.downloads, "categories": p.categories,
        "version": newest, "support": support,
        "installed": installed.is_some(),
        "update": installed.is_some_and(|i| !newest.is_empty() && i.version != newest),
    })
}

#[tauri::command]
pub async fn horizon_search(instance_id: String, query: String, show_native: bool, offset: usize) -> CmdResult<Value> {
    let inst = load_rust_instance(&instance_id)?;
    let (projects, revoked) = library().await?;
    let items = read_items(&instance_id);
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
    let built: HashSet<String> = crate::rust::builtin_ids().into_iter().collect();
    let mut found: Vec<&Project> = projects
        .iter()
        .filter(|p| p.kind == "mod" || (p.kind == "native-mod" && (show_native || built.contains(&p.id))))
        .filter(|p| {
            let hay = format!("{} {} {} {} {}", p.id, p.name, p.summary, p.authors.join(" "), p.categories.join(" ")).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect();
    // What is in the game first, then what can be installed, then the rest; the most downloaded first.
    let rank = |p: &Project| match support(p, &revoked, &inst.mc_version) { "builtin" => 0, "ok" => 1, _ => 2 };
    found.sort_by_key(|p| (rank(p), std::cmp::Reverse(p.downloads)));
    let total = found.len();
    let hits: Vec<Value> = found
        .into_iter()
        .skip(offset)
        .take(PAGE)
        .map(|p| hit(p, support(p, &revoked, &inst.mc_version), items.iter().find(|i| i.id == p.id), &inst.mc_version, &revoked))
        .collect();
    let hits: Vec<Value> = hits
        .into_iter()
        .map(|mut h| {
            if h["support"] == "builtin" {
                h["on"] = json!(native_on(&instance_id, h["id"].as_str().unwrap_or("")));
            }
            h
        })
        .collect();
    Ok(json!({ "hits": hits, "total": total }))
}

#[tauri::command]
pub async fn horizon_project(instance_id: String, id: String) -> CmdResult<Value> {
    let inst = load_rust_instance(&instance_id)?;
    let (projects, revoked) = library().await?;
    let p = projects.iter().find(|p| p.id == id).ok_or_else(|| anyhow!("That mod is not in the library"))?;
    let items = read_items(&instance_id);
    let mut v = hit(p, support(p, &revoked, &inst.mc_version), items.iter().find(|i| i.id == p.id), &inst.mc_version, &revoked);
    v["description"] = json!(p.description);
    v["license"] = json!(p.license);
    v["source"] = json!(p.source);
    v["page"] = json!(format!("{SITE}/{}", p.id));
    if v["support"] == "builtin" {
        v["on"] = json!(native_on(&instance_id, &p.id));
    }
    Ok(v)
}

#[tauri::command]
pub async fn horizon_install(instance_id: String, id: String) -> CmdResult<String> {
    let inst = load_rust_instance(&instance_id)?;
    let (projects, revoked) = library().await?;
    let p = projects.iter().find(|p| p.id == id).ok_or_else(|| anyhow!("That mod is not in the library"))?;
    install_one(&instance_id, &inst.mc_version, p, &revoked).await?;
    Ok(p.name.clone())
}

/// The native mods built into the installed client, with whether each is on for this instance.
#[tauri::command]
pub fn horizon_builtin(instance_id: String) -> CmdResult<Vec<Value>> {
    paths::safe_id(&instance_id)?;
    Ok(crate::rust::builtin_mods()
        .into_iter()
        .map(|m| {
            let id = m["id"].as_str().unwrap_or("").to_string();
            json!({
                "project_id": id, "title": m["name"], "version_name": m["version"], "icon": m["icon"],
                "summary": m["summary"], "kind": "mod", "source": "native", "enabled": native_on(&instance_id, &id),
            })
        })
        .collect())
}

/// Switches a built-in mod on or off for the instance (takes effect the next time the game starts).
#[tauri::command]
pub fn horizon_native_toggle(instance_id: String, id: String) -> CmdResult<bool> {
    paths::safe_id(&instance_id)?;
    if !crate::rust::builtin_ids().contains(&id) {
        return Err(anyhow!("That mod is not built into this game").into());
    }
    let on = !native_on(&instance_id, &id);
    set_native(&instance_id, &id, on)?;
    Ok(on)
}

#[tauri::command]
pub fn horizon_installed(instance_id: String) -> CmdResult<Vec<Value>> {
    paths::safe_id(&instance_id)?;
    let dir = mods_dir(&instance_id);
    Ok(read_items(&instance_id)
        .into_iter()
        .filter(|i| dir.join(&i.filename).exists())
        .map(|i| json!({
            "project_id": i.id, "title": i.title, "version_name": i.version, "icon": i.icon,
            "filename": i.filename, "kind": "mod", "source": "horizon", "enabled": is_enabled(&instance_id, &i.id),
        }))
        .collect())
}

#[tauri::command]
pub fn horizon_toggle(instance_id: String, id: String) -> CmdResult<bool> {
    paths::safe_id(&instance_id)?;
    if !read_items(&instance_id).iter().any(|i| i.id == id) {
        return Err(anyhow!("That mod is not installed").into());
    }
    let on = !is_enabled(&instance_id, &id);
    set_enabled(&instance_id, &id, on)?;
    Ok(on)
}

#[tauri::command]
pub fn horizon_remove(instance_id: String, id: String) -> CmdResult<()> {
    paths::safe_id(&instance_id)?;
    let mut items = read_items(&instance_id);
    if let Some(i) = items.iter().find(|i| i.id == id) {
        paths::safe_file_name(&i.filename)?;
        let _ = std::fs::remove_file(mods_dir(&instance_id).join(&i.filename));
    }
    items.retain(|i| i.id != id);
    write_items(&instance_id, &items)?;
    Ok(set_enabled(&instance_id, &id, false)?)
}

/// Install the newest version of everything installed that has a newer one; returns how many.
#[tauri::command]
pub async fn horizon_update_all(instance_id: String) -> CmdResult<usize> {
    let inst = load_rust_instance(&instance_id)?;
    let (projects, revoked) = library().await?;
    let mut n = 0;
    for item in read_items(&instance_id) {
        let Some(p) = projects.iter().find(|p| p.id == item.id) else { continue };
        let Some(v) = pick(p, &revoked, &inst.mc_version) else { continue };
        if v.file.sha256.eq_ignore_ascii_case(&item.sha256) {
            continue;
        }
        let was_on = is_enabled(&instance_id, &item.id);
        install_one(&instance_id, &inst.mc_version, p, &revoked).await?;
        if !was_on {
            set_enabled(&instance_id, &item.id, false)?;
        }
        n += 1;
    }
    Ok(n)
}
