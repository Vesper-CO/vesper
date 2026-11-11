//! Tauri commands for the Performance section of the instance editor: a preview of what the launcher
//! will do (memory, JVM flags), and one-click installation of well-known performance mods.
use crate::{error::CmdResult, instances, paths, perf};
use anyhow::anyhow;
use serde::Serialize;
use tauri::AppHandle;

/// Each entry is "install the first of these that works for this Minecraft version".
type ModSet = &'static [&'static [&'static str]];
const FABRIC: ModSet = &[&["sodium"], &["lithium"], &["ferrite-core"], &["modernfix"], &["entityculling"], &["immediatelyfast"], &["memoryleakfix"], &["dynamic-fps"]];
const FORGE: ModSet = &[&["embeddium"], &["modernfix"], &["ferrite-core"], &["entityculling"], &["immediatelyfast"]];
const NEOFORGE: ModSet = &[&["sodium", "embeddium"], &["modernfix"], &["ferrite-core"], &["entityculling"], &["immediatelyfast"]];

pub fn mod_count(id: &str) -> usize {
    std::fs::read_dir(paths::game_dir(id).join("mods"))
        .map(|rd| rd.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".jar")).count())
        .unwrap_or(0)
}

#[derive(Serialize)]
pub struct PerfInfo {
    system_mb: u64,
    recommended_mb: u32,
    mods: usize,
    java_major: u32,
    plan: perf::Plan,
}

/// What would happen at launch with these settings. The Java version is a guess (the real one comes
/// from Mojang's version file when the game starts), which the UI says.
#[tauri::command]
pub fn perf_info(id: Option<String>, mc_version: String, loader: String, profile: String, ram_mb: u32) -> CmdResult<PerfInfo> {
    let total = perf::total_ram_mb();
    let mods = id.as_deref().filter(|i| paths::safe_id(i).is_ok()).map(mod_count).unwrap_or(0);
    let java_major = perf::java_major_for(&mc_version);
    Ok(PerfInfo { system_mb: total, recommended_mb: perf::recommend_ram_mb(total, &loader, mods), mods, java_major, plan: perf::plan(&profile, ram_mb, java_major, total) })
}

#[derive(Serialize)]
pub struct Optimized {
    installed: Vec<String>,
    unavailable: Vec<String>,
}

/// Install the performance mods that exist for this instance's Minecraft version and loader.
#[tauri::command]
pub async fn optimize_instance(app: AppHandle, id: String) -> CmdResult<Optimized> {
    let inst = instances::load(&id)?;
    let set = match inst.loader.as_str() {
        "fabric" | "quilt" => FABRIC,
        "forge" => FORGE,
        "neoforge" => NEOFORGE,
        "rust" => return Err(anyhow!("Rust instances do not use Java, so there is nothing to optimize here.").into()),
        _ => return Err(anyhow!("Performance mods need a mod loader. Edit this instance and pick Fabric: it has the best FPS mods. The memory and Java tuning already applies to vanilla.").into()),
    };
    let (mut installed, mut unavailable) = (vec![], vec![]);
    for alts in set {
        let mut done = false;
        for slug in *alts {
            // Dependencies (such as Fabric API) come along automatically.
            if let Ok(names) = crate::modrinth::modrinth_install(app.clone(), id.clone(), slug.to_string(), None).await {
                installed.extend(names);
                done = true;
                break;
            }
        }
        if !done {
            unavailable.push(alts[0].to_string());
        }
    }
    if installed.is_empty() {
        return Err(anyhow!("None of the performance mods support {} {} yet.", inst.loader, inst.mc_version).into());
    }
    installed.dedup();
    Ok(Optimized { installed, unavailable })
}
