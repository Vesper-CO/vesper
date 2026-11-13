#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod download;
mod error;
mod horizon_mods;
mod hud;
mod hud_rust;
mod instances;
mod launch;
mod loaders;
mod modrinth;
mod mojang;
mod mrpack;
mod optimize;
mod paths;
mod perf;
mod presence;
mod rust;
mod settings;

use error::CmdResult;
use tauri::{AppHandle, Manager};

/// False when the updater plugin could not start (e.g. the public key is still a placeholder).
struct UpdaterReady(bool);

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    if !url.starts_with("https://") {
        return Err(anyhow::anyhow!("Only https links can be opened").into());
    }
    app.opener().open_url(url, None::<&str>)?;
    Ok(())
}

#[tauri::command]
fn open_data_folder(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(paths::root().to_string_lossy().to_string(), None::<&str>)?;
    Ok(())
}

#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    current: String,
    notes: String,
}

/// Returns details of a newer version if one is available.
#[tauri::command]
async fn check_update(app: AppHandle) -> CmdResult<Option<UpdateInfo>> {
    use tauri_plugin_updater::UpdaterExt;
    if !app.state::<UpdaterReady>().0 {
        return Ok(None);
    }
    match app.updater()?.check().await {
        Ok(u) => Ok(u.map(|u| UpdateInfo { version: u.version, current: u.current_version, notes: u.body.unwrap_or_default() })),
        // The latest release has no latest.json yet (for example a hand-made release): nothing to offer.
        Err(tauri_plugin_updater::Error::ReleaseNotFound) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Download, install and restart into the new version.
#[tauri::command]
async fn install_update(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_updater::UpdaterExt;
    if !app.state::<UpdaterReady>().0 {
        return Err(anyhow::anyhow!("Updates are not configured in this build").into());
    }
    if let Some(u) = app.updater()?.check().await? {
        u.download_and_install(|_, _| {}, || {}).await?;
        app.restart();
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(launch::Running::default())
        .setup(|app| {
            let ok = app.handle().plugin(tauri_plugin_updater::Builder::new().build()).is_ok();
            app.manage(UpdaterReady(ok));
            presence::start(settings::load().discord_presence);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_url,
            open_data_folder,
            check_update,
            install_update,
            settings::get_settings,
            settings::save_settings,
            mojang::list_mc_versions,
            mojang::get_news,
            loaders::loader_versions,
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::open_instance_folder,
            auth::login_start,
            auth::login_finish,
            auth::login_cancel,
            auth::list_accounts,
            auth::set_active_account,
            auth::remove_account,
            auth::refresh_profile,
            launch::play,
            launch::kill_game,
            launch::running_instances,
            presence::presence_available,
            presence::presence_page,
            horizon_mods::horizon_search,
            horizon_mods::horizon_project,
            horizon_mods::horizon_install,
            horizon_mods::horizon_installed,
            horizon_mods::horizon_builtin,
            horizon_mods::horizon_native_toggle,
            horizon_mods::horizon_toggle,
            horizon_mods::horizon_remove,
            horizon_mods::horizon_update_all,
            modrinth::modrinth_categories,
            modrinth::modrinth_search,
            modrinth::modrinth_project,
            modrinth::modrinth_versions,
            modrinth::modrinth_install,
            modrinth::modrinth_install_modpack,
            modrinth::import_mrpack,
            modrinth::modrinth_installed,
            modrinth::modrinth_toggle,
            modrinth::modrinth_remove,
            modrinth::modrinth_update_all,
            optimize::perf_info,
            optimize::optimize_instance,
            hud::get_hud,
            hud::save_hud,
            hud::hud_install,
            hud::hud_set,
        ])
        .run(tauri::generate_context!())
        .expect("Vesper failed to start");
}
