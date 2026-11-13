use crate::{error::CmdResult, paths};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    /// The Christmas theme (snow, trees, red and green) on top of dark or light.
    pub christmas: bool,
    pub default_ram_mb: u32,
    /// Optional: use this java.exe instead of Mojang's runtime.
    pub java_path: String,
    pub auto_update: bool,
    /// Show what is being played on the player's Discord profile.
    pub discord_presence: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "dark".into(), christmas: false, default_ram_mb: 4096, java_path: String::new(), auto_update: true, discord_presence: true }
    }
}

pub fn load() -> Settings {
    std::fs::read(paths::settings_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn get_settings() -> Settings { load() }

#[tauri::command]
pub fn save_settings(settings: Settings) -> CmdResult<()> {
    std::fs::write(paths::settings_file(), serde_json::to_vec_pretty(&settings)?)?;
    crate::presence::set_enabled(settings.discord_presence);
    Ok(())
}
