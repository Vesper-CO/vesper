//! Everything that comes from Mojang's official endpoints: version manifest, version JSON,
//! client jar, libraries, asset index/objects and the Java runtimes.
//! Nothing is bundled with Vesper - it is all downloaded on the user's machine.
use crate::{download::*, error::CmdResult, paths};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use tauri::AppHandle;

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_ALL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const NEWS: &str = "https://launchercontent.mojang.com/v2/javaPatchNotes.json";

#[tauri::command]
pub async fn list_mc_versions() -> CmdResult<Vec<Value>> {
    let m: Value = get_json(MANIFEST).await?;
    Ok(m["versions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| json!({"id": v["id"], "type": v["type"], "releaseTime": v["releaseTime"]}))
        .collect())
}

#[tauri::command]
pub async fn get_news() -> CmdResult<Vec<Value>> {
    let v: Value = get_json(NEWS).await?;
    let mut e = v["entries"].as_array().cloned().unwrap_or_default();
    e.sort_by(|a, b| b["date"].as_str().unwrap_or("").cmp(a["date"].as_str().unwrap_or("")));
    Ok(e.into_iter()
        .take(6)
        .map(|x| {
            let img = x["image"]["url"].as_str().unwrap_or("");
            let img = if img.starts_with("http") { img.to_string() } else if img.is_empty() { String::new() } else { format!("https://launchercontent.mojang.com/{}", img.trim_start_matches('/')) };
            json!({"title": x["title"], "version": x["version"], "type": x["type"], "date": x["date"], "image": img})
        })
        .collect())
}

/// Load `versions/<id>/<id>.json`, downloading it from the manifest if it is a Mojang version.
pub async fn load_version(id: &str) -> Result<Value> {
    let p = paths::version_json(id);
    if !p.exists() {
        let m: Value = get_json(MANIFEST).await?;
        let e = m["versions"].as_array().and_then(|a| a.iter().find(|v| v["id"] == id)).ok_or_else(|| anyhow!("Unknown Minecraft version: {id}"))?;
        let url = e["url"].as_str().ok_or_else(|| anyhow!("Manifest entry has no url"))?.to_string();
        let sha1 = e["sha1"].as_str().map(String::from);
        download_one(&Job { url, path: p.clone(), sha1, size: None }).await?;
    }
    Ok(serde_json::from_slice(&tokio::fs::read(&p).await?)?)
}

/// Resolve `inheritsFrom` (Fabric/Quilt/Forge profiles) into one merged version document.
/// Child libraries come first so they win when the classpath is de-duplicated.
pub async fn resolve(id: &str) -> Result<Value> {
    let v = load_version(id).await?;
    let Some(parent) = v["inheritsFrom"].as_str().map(String::from) else { return Ok(v) };
    let mut p = Box::pin(resolve(&parent)).await?;
    for (k, val) in v.as_object().cloned().unwrap_or_default() {
        match k.as_str() {
            "libraries" => {
                let mut libs = val.as_array().cloned().unwrap_or_default();
                libs.extend(p["libraries"].as_array().cloned().unwrap_or_default());
                p["libraries"] = Value::Array(libs);
            }
            "arguments" => {
                for sect in ["game", "jvm"] {
                    let mut a = p["arguments"][sect].as_array().cloned().unwrap_or_default();
                    a.extend(val[sect].as_array().cloned().unwrap_or_default());
                    p["arguments"][sect] = Value::Array(a);
                }
            }
            _ => p[k.as_str()] = val,
        }
    }
    Ok(p)
}

/// The id whose `.jar` is the actual Minecraft client.
pub fn jar_id(v: &Value) -> String {
    v["inheritsFrom"].as_str().or(v["id"].as_str()).unwrap_or_default().to_string()
}

// ---------- rules ----------

/// Mojang rule evaluation. We are Windows x64 with no optional features (demo mode, custom size...).
pub fn rules_ok(rules: Option<&Value>) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else { return true };
    let mut allowed = false;
    for r in rules {
        if rule_matches(r) {
            allowed = r["action"] == "allow";
        }
    }
    allowed
}

fn rule_matches(r: &Value) -> bool {
    if let Some(os) = r.get("os") {
        if let Some(n) = os["name"].as_str() {
            if n != "windows" {
                return false;
            }
        }
        if os.get("arch").is_some() {
            return false;
        }
    }
    if let Some(f) = r.get("features").and_then(|f| f.as_object()) {
        if f.values().any(|req| req.as_bool() == Some(true)) {
            return false;
        }
    }
    true
}

// ---------- libraries ----------

#[derive(Default)]
pub struct Libs {
    pub jobs: Vec<Job>,
    pub classpath: Vec<PathBuf>,
    /// Old-style native jars that must be extracted (Minecraft 1.12 and older).
    pub natives: Vec<PathBuf>,
}

fn art(a: &Value) -> Option<(Option<Job>, PathBuf)> {
    let path = paths::libraries().join(a["path"].as_str()?);
    let url = a["url"].as_str().unwrap_or("");
    let job = (!url.is_empty()).then(|| Job { url: url.into(), path: path.clone(), sha1: a["sha1"].as_str().map(String::from), size: a["size"].as_u64() });
    Some((job, path))
}

/// Libraries that only carry a Maven coordinate (Fabric/Quilt) instead of a `downloads` block.
fn maven(l: &Value, name: &str) -> Option<(Option<Job>, PathBuf)> {
    let (coords, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let p: Vec<&str> = coords.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    let (g, a, v) = (p[0].replace('.', "/"), p[1], p[2]);
    let cls = p.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    let rel = format!("{g}/{a}/{v}/{a}-{v}{cls}.{ext}");
    let base = l["url"].as_str().unwrap_or("https://libraries.minecraft.net/");
    let url = format!("{}{}{}", base, if base.ends_with('/') { "" } else { "/" }, rel);
    let path = paths::libraries().join(&rel);
    Some((Some(Job { url, path: path.clone(), sha1: l["sha1"].as_str().map(String::from), size: l["size"].as_u64() }), path))
}

pub fn collect_libs(v: &Value) -> Libs {
    let mut out = Libs::default();
    let mut seen = HashSet::new();
    for l in v["libraries"].as_array().into_iter().flatten() {
        if !rules_ok(l.get("rules")) {
            continue;
        }
        let name = l["name"].as_str().unwrap_or("");
        if let Some(cls) = l["natives"]["windows"].as_str() {
            let cls = cls.replace("${arch}", "64");
            if let Some((job, path)) = l["downloads"]["classifiers"].get(&cls).and_then(art) {
                out.jobs.extend(job);
                out.natives.push(path);
            }
        }
        let p: Vec<&str> = name.split(':').collect();
        let key = format!("{}:{}:{}", p.first().unwrap_or(&""), p.get(1).unwrap_or(&""), p.get(3).unwrap_or(&""));
        if !seen.insert(key) {
            continue;
        }
        let found = if let Some(a) = l["downloads"].get("artifact") { art(a) } else if l["downloads"].is_null() { maven(l, name) } else { None };
        if let Some((job, path)) = found {
            out.jobs.extend(job);
            out.classpath.push(path);
        }
    }
    out
}

/// Download the client jar and all libraries.
pub async fn ensure_game(app: &AppHandle, v: &Value) -> Result<Libs> {
    let libs = collect_libs(v);
    let c = &v["downloads"]["client"];
    let mut jobs = libs.jobs.clone();
    if let Some(url) = c["url"].as_str() {
        jobs.push(Job { url: url.into(), path: paths::version_jar(&jar_id(v)), sha1: c["sha1"].as_str().map(String::from), size: c["size"].as_u64() });
    }
    download_all(app, "Game files", jobs).await?;
    Ok(libs)
}

// ---------- assets ----------

pub struct AssetInfo {
    pub index_id: String,
    /// Value for ${game_assets}: only differs from the assets root for pre-1.7 versions.
    pub game_assets: PathBuf,
}

pub async fn ensure_assets(app: &AppHandle, v: &Value, game_dir: &Path) -> Result<AssetInfo> {
    let ai = &v["assetIndex"];
    let id = ai["id"].as_str().ok_or_else(|| anyhow!("Version has no asset index"))?.to_string();
    let index_path = paths::assets().join("indexes").join(format!("{id}.json"));
    download_one(&Job { url: ai["url"].as_str().unwrap_or_default().into(), path: index_path.clone(), sha1: ai["sha1"].as_str().map(String::from), size: None }).await?;
    let index: Value = serde_json::from_slice(&tokio::fs::read(&index_path).await?)?;
    let objects = index["objects"].as_object().cloned().unwrap_or_default();

    let mut by_hash: HashMap<String, u64> = HashMap::new();
    for o in objects.values() {
        if let Some(h) = o["hash"].as_str() {
            by_hash.insert(h.to_string(), o["size"].as_u64().unwrap_or(0));
        }
    }
    let jobs = by_hash
        .into_iter()
        .map(|(h, size)| Job {
            url: format!("https://resources.download.minecraft.net/{}/{}", &h[..2], h),
            path: paths::assets().join("objects").join(&h[..2]).join(&h),
            sha1: Some(h),
            size: Some(size),
        })
        .collect();
    download_all(app, "Assets", jobs).await?;

    let virtual_ = index["virtual"].as_bool().unwrap_or(false);
    let to_resources = index["map_to_resources"].as_bool().unwrap_or(false);
    let mut game_assets = paths::assets();
    if virtual_ || to_resources {
        let dest = if to_resources { game_dir.join("resources") } else { paths::assets().join("virtual").join(&id) };
        let objs = objects.clone();
        let dest2 = dest.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            for (name, o) in objs {
                let h = o["hash"].as_str().unwrap_or_default();
                let target = dest2.join(&name);
                if h.len() < 2 || target.exists() || name.contains("..") {
                    continue;
                }
                if let Some(p) = target.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::copy(paths::assets().join("objects").join(&h[..2]).join(h), target)?;
            }
            Ok(())
        })
        .await??;
        game_assets = dest;
    }
    Ok(AssetInfo { index_id: id, game_assets })
}

// ---------- Java ----------

/// Download the Java runtime Mojang ships for this Minecraft version (Java 8/17/21...).
pub async fn ensure_java(app: &AppHandle, v: &Value) -> Result<PathBuf> {
    let custom = crate::settings::load().java_path;
    if !custom.trim().is_empty() {
        return Ok(PathBuf::from(custom.trim()));
    }
    let comp = v["javaVersion"]["component"].as_str().unwrap_or("jre-legacy").to_string();
    let dir = paths::runtimes().join(&comp);
    let exe = dir.join("bin").join("java.exe");
    let marker = dir.join(".vesper-ok");
    if marker.exists() && exe.exists() {
        return Ok(exe);
    }
    let all: Value = get_json(JAVA_ALL).await?;
    let entry = &all["windows-x64"][&comp][0]["manifest"];
    let url = entry["url"].as_str().ok_or_else(|| anyhow!("Mojang has no Java runtime '{comp}' for Windows x64"))?;
    let man: Value = get_json(url).await?;
    let mut jobs = vec![];
    for (rel, f) in man["files"].as_object().into_iter().flatten() {
        if f["type"] == "file" {
            if let Some(raw) = f["downloads"].get("raw") {
                jobs.push(Job { url: raw["url"].as_str().unwrap_or_default().into(), path: dir.join(rel), sha1: raw["sha1"].as_str().map(String::from), size: raw["size"].as_u64() });
            }
        }
    }
    if jobs.is_empty() {
        bail!("Java runtime manifest was empty");
    }
    download_all(app, "Java runtime", jobs).await?;
    tokio::fs::write(&marker, b"ok").await?;
    Ok(exe)
}
