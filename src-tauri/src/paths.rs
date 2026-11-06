//! All on-disk locations. Everything lives under %APPDATA%\Vesper (per-user, no admin needed).
//!
//!   shared\    versions, libraries, assets - shared by every instance
//!   runtimes\  Java runtimes downloaded from Mojang
//!   instances\<id>\instance.json + game\ (the instance's .minecraft)
use anyhow::{bail, Result};
use std::{path::PathBuf, sync::OnceLock};

static ROOT: OnceLock<PathBuf> = OnceLock::new();

pub fn root() -> &'static PathBuf {
    ROOT.get_or_init(|| {
        let p = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("Vesper");
        let _ = std::fs::create_dir_all(&p);
        p
    })
}
pub fn shared() -> PathBuf { root().join("shared") }
pub fn libraries() -> PathBuf { shared().join("libraries") }
pub fn assets() -> PathBuf { shared().join("assets") }
pub fn versions() -> PathBuf { shared().join("versions") }
pub fn runtimes() -> PathBuf { root().join("runtimes") }
pub fn instances() -> PathBuf { root().join("instances") }
pub fn tmp() -> PathBuf { root().join("tmp") }
pub fn version_json(id: &str) -> PathBuf { versions().join(id).join(format!("{id}.json")) }
pub fn version_jar(id: &str) -> PathBuf { versions().join(id).join(format!("{id}.jar")) }
pub fn instance_dir(id: &str) -> PathBuf { instances().join(id) }
pub fn game_dir(id: &str) -> PathBuf { instance_dir(id).join("game") }
pub fn accounts_file() -> PathBuf { root().join("accounts.json") }
pub fn settings_file() -> PathBuf { root().join("settings.json") }

/// Instance ids come from the UI; never let them escape the instances folder.
pub fn safe_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        bail!("Invalid instance id");
    }
    Ok(())
}

/// A file name received from a remote API or the UI: no folders allowed.
pub fn safe_file_name(n: &str) -> Result<()> {
    if n.is_empty() || n.contains('/') || n.contains('\\') || n.contains("..") || n.contains(':') {
        bail!("Unsafe file name: {n}");
    }
    Ok(())
}
