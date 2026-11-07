//! Mod loaders. Fabric and Quilt publish ready-made launch profiles; Forge and NeoForge ship an
//! installer jar which we run headless (`--installClient`) with the right Java.
use crate::{download::*, instances::Instance, mojang, paths};
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command};

const FABRIC: &str = "https://meta.fabricmc.net/v2/versions/loader";
const QUILT: &str = "https://meta.quiltmc.org/v3/versions/loader";
const FORGE_PROMOS: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_VERSIONS: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// Loader versions available for a Minecraft version, best first. Empty = not supported.
pub async fn available(loader: &str, mc: &str) -> Result<Vec<String>> {
    Ok(match loader {
        "fabric" | "quilt" => {
            let base = if loader == "fabric" { FABRIC } else { QUILT };
            let arr: Vec<Value> = get_json(&format!("{base}/{mc}")).await.unwrap_or_default();
            let all: Vec<(String, bool)> = arr
                .iter()
                .filter_map(|x| Some((x["loader"]["version"].as_str()?.to_string(), x["loader"]["stable"].as_bool().unwrap_or(!x["loader"]["version"].as_str()?.contains('-')))))
                .collect();
            let stable: Vec<String> = all.iter().filter(|(_, s)| *s).map(|(v, _)| v.clone()).collect();
            if stable.is_empty() { all.into_iter().map(|(v, _)| v).collect() } else { stable }
        }
        "forge" => {
            let p: Value = get_json(FORGE_PROMOS).await?;
            let mut out: Vec<String> = vec![];
            for k in [format!("{mc}-recommended"), format!("{mc}-latest")] {
                if let Some(v) = p["promos"][&k].as_str() {
                    if !out.iter().any(|o| o == v) {
                        out.push(v.to_string());
                    }
                }
            }
            out
        }
        "neoforge" => {
            // NeoForge versions are "<minor>.<patch>.<build>" for Minecraft 1.<minor>.<patch>.
            let parts: Vec<&str> = mc.split('.').collect();
            if parts.len() < 2 || parts[0] != "1" {
                return Ok(vec![]);
            }
            let prefix = format!("{}.{}.", parts[1], parts.get(2).unwrap_or(&"0"));
            let v: Value = get_json(NEOFORGE_VERSIONS).await?;
            let mut all: Vec<String> = v["versions"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).filter(|x| x.starts_with(&prefix)).map(String::from).collect();
            all.reverse();
            let stable: Vec<String> = all.iter().filter(|x| !x.contains('-')).cloned().collect();
            if stable.is_empty() { all } else { stable }
        }
        _ => vec![],
    })
}

#[tauri::command]
pub async fn loader_versions(loader: String, mc: String) -> Result<Vec<String>, crate::error::CmdError> {
    Ok(available(&loader, &mc).await?)
}

/// Make sure the instance's launch profile exists in shared/versions. Sets `version_id`.
pub async fn install(app: &AppHandle, inst: &mut Instance) -> Result<()> {
    let mc = inst.mc_version.clone();
    match inst.loader.as_str() {
        "vanilla" => {
            mojang::load_version(&mc).await?;
            inst.version_id = mc;
        }
        "fabric" | "quilt" => {
            let base = if inst.loader == "fabric" { FABRIC } else { QUILT };
            let lv = if inst.loader_version.is_empty() {
                available(&inst.loader, &mc).await?.first().cloned().ok_or_else(|| anyhow!("{} does not support Minecraft {mc} yet", inst.loader))?
            } else {
                inst.loader_version.clone()
            };
            mojang::load_version(&mc).await?;
            let profile: Value = get_json(&format!("{base}/{mc}/{lv}/profile/json")).await?;
            let id = profile["id"].as_str().ok_or_else(|| anyhow!("Loader profile has no id"))?.to_string();
            paths::safe_file_name(&id)?;
            let p = paths::version_json(&id);
            tokio::fs::create_dir_all(p.parent().unwrap()).await?;
            tokio::fs::write(&p, serde_json::to_vec_pretty(&profile)?).await?;
            inst.version_id = id;
            inst.loader_version = lv;
        }
        "forge" | "neoforge" => install_with_installer(app, inst).await?,
        "rust" => inst.version_id = "rust".into(), // Rust instances never use a Java version profile
        other => bail!("Unknown loader {other}"),
    }
    Ok(())
}

async fn install_with_installer(app: &AppHandle, inst: &mut Instance) -> Result<()> {
    let mc = inst.mc_version.clone();
    let neo = inst.loader == "neoforge";
    let lv = if inst.loader_version.is_empty() {
        available(&inst.loader, &mc).await?.first().cloned().ok_or_else(|| anyhow!("{} has no build for Minecraft {mc}", inst.loader))?
    } else {
        inst.loader_version.clone()
    };
    let (url, file) = if neo {
        (format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar"), format!("neoforge-{lv}-installer.jar"))
    } else {
        let full = format!("{mc}-{lv}");
        (format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"), format!("forge-{full}-installer.jar"))
    };

    // The installer needs vanilla installed first, and a Java runtime.
    let vanilla = mojang::resolve(&mc).await?;
    mojang::ensure_game(app, &vanilla).await?;
    let java = mojang::ensure_java(app, &vanilla).await?;

    let shared = paths::shared();
    tokio::fs::create_dir_all(&shared).await?;
    let profiles = shared.join("launcher_profiles.json");
    if !profiles.exists() {
        tokio::fs::write(&profiles, br#"{"profiles":{},"version":3}"#).await?;
    }
    let installer: PathBuf = paths::tmp().join(&file);
    download_one(&Job { url, path: installer.clone(), sha1: None, size: None }).await?;

    let _ = app.emit("launch-stage", serde_json::json!({"id": inst.id, "stage": format!("Installing {}", inst.loader)}));
    let mut child = Command::new(&java)
        .arg("-jar").arg(&installer).arg("--installClient").arg(&shared)
        .current_dir(&shared)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let app2 = app.clone();
    let tail = tokio::spawn(async move {
        let mut last = vec![];
        while let Ok(Some(l)) = lines.next_line().await {
            let _ = app2.emit("install-log", &l);
            last.push(l);
            if last.len() > 12 { last.remove(0); }
        }
        last
    });
    let status = child.wait().await?;
    let last = tail.await.unwrap_or_default();
    if !status.success() {
        bail!("The {} installer failed:\n{}", inst.loader, last.join("\n"));
    }
    let _ = tokio::fs::remove_file(&installer).await;

    // Find the profile the installer created.
    let needle = if neo { "neoforge" } else { "forge" };
    let mut found = None;
    for e in std::fs::read_dir(paths::versions())?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.contains(needle) && n.contains(&lv) && paths::version_json(&n).exists() {
            found = Some(n);
        }
    }
    inst.version_id = found.ok_or_else(|| anyhow!("The installer finished but no launch profile was created"))?;
    inst.loader_version = lv;
    Ok(())
}
