use crate::{error::CmdResult, paths};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub default_ram_mb: u32,
    /// Optional: use this java.exe instead of Mojang's runtime.
    pub java_path: String,
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme: "dark".into(), default_ram_mb: 4096, java_path: String::new(), auto_update: true }
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
    Ok(())
}
