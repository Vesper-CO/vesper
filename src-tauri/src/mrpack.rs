//! Import `.mrpack` modpacks (Modrinth's open format) as a new instance.
use crate::{download::*, instances::{self, Instance}, paths};
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::{
    io::Read,
    path::{Component, Path, PathBuf},
};
use tauri::AppHandle;

/// A path from inside a pack must stay inside the instance folder.
fn safe_rel(p: &str) -> Result<PathBuf> {
    let path = PathBuf::from(p);
    if path.components().all(|c| matches!(c, Component::Normal(_))) && !p.is_empty() {
        Ok(path)
    } else {
        bail!("Modpack contains an unsafe path: {p}")
    }
}

pub async fn import_path(app: &AppHandle, pack: &Path) -> Result<Instance> {
    let p = pack.to_owned();
    let index: Value = tokio::task::spawn_blocking(move || -> Result<Value> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(p)?)?;
        let mut s = String::new();
        z.by_name("modrinth.index.json").map_err(|_| anyhow!("Not a valid .mrpack (modrinth.index.json missing)"))?.read_to_string(&mut s)?;
        Ok(serde_json::from_str(&s)?)
    })
    .await??;

    let deps = &index["dependencies"];
    let mc = deps["minecraft"].as_str().ok_or_else(|| anyhow!("Modpack does not say which Minecraft version it needs"))?;
    let (loader, lv) = if let Some(v) = deps["fabric-loader"].as_str() { ("fabric", v) }
        else if let Some(v) = deps["quilt-loader"].as_str() { ("quilt", v) }
        else if let Some(v) = deps["neoforge"].as_str() { ("neoforge", v) }
        else if let Some(v) = deps["forge"].as_str() { ("forge", v) }
        else { ("vanilla", "") };
    let name = index["name"].as_str().unwrap_or("Imported modpack");
    let inst = instances::create(name, mc, loader, Some(lv.to_string()), 4096)?;
    let game = paths::game_dir(&inst.id);

    let mut jobs = vec![];
    for f in index["files"].as_array().into_iter().flatten() {
        if f["env"]["client"] == "unsupported" {
            continue;
        }
        let rel = safe_rel(f["path"].as_str().unwrap_or(""))?;
        let url = f["downloads"][0].as_str().ok_or_else(|| anyhow!("A modpack file has no download url"))?;
        jobs.push(Job { url: url.into(), path: game.join(rel), sha1: f["hashes"]["sha1"].as_str().map(String::from), size: f["fileSize"].as_u64() });
    }
    download_all(app, "Modpack files", jobs).await?;

    // overrides/ and client-overrides/ are copied over the game folder (configs, options, ...).
    let (pack, game2) = (pack.to_owned(), game.clone());
    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(pack)?)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let Some(name) = e.enclosed_name() else { continue };
            let rel = match name.strip_prefix("overrides").or_else(|_| name.strip_prefix("client-overrides")) {
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
    Ok(inst)
}
