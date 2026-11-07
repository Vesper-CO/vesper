//! Vesper Setup: a small branded installer.
//!
//! It carries the real Vesper installer (the one Tauri builds, which is also what the in-app
//! updater uses) inside itself and runs it silently, so the install location, Start Menu entry,
//! uninstaller and update path are exactly the same as a normal Vesper install.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};
use tauri::{AppHandle, Emitter};

static PAYLOAD: &[u8] = include_bytes!("../payload/vesper-setup.exe");

/// Run helper programs without flashing a console window.
fn quiet(mut c: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c.stdout(Stdio::null()).stderr(Stdio::null());
    c
}

fn stage(app: &AppHandle, s: &str) {
    let _ = app.emit("stage", s);
}

/// Where the per-user Vesper install ended up, if anywhere.
fn installed_exe() -> Option<PathBuf> {
    let local = dirs::data_local_dir()?;
    for dir in [local.join("Vesper"), local.join("Programs").join("Vesper")] {
        let main = dir.join("vesper.exe"); // Windows file names are case-insensitive
        if main.exists() {
            return Some(main);
        }
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_lowercase();
                if n.ends_with(".exe") && !n.starts_with("uninstall") {
                    return Some(e.path());
                }
            }
        }
    }
    None
}

fn make_shortcut(lnk: &Path, exe: &Path) -> Result<(), String> {
    let q = |p: &Path| p.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Description='Vesper';$s.Save()",
        q(lnk), q(exe), q(exe.parent().unwrap_or(exe))
    );
    let mut c = quiet(Command::new("powershell"));
    c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script]);
    match c.status() {
        Ok(s) if s.success() => Ok(()),
        _ => Err("Could not create a shortcut".into()),
    }
}

fn run_install(app: &AppHandle, desktop: bool) -> Result<String, String> {
    if PAYLOAD.len() < 1_000_000 {
        return Err("This installer was built without the Vesper app inside it.".into());
    }
    stage(app, "prepare");
    // An open copy of Vesper would block the file replacement.
    if installed_exe().is_some() {
        let _ = quiet(Command::new("taskkill")).args(["/F", "/IM", "vesper.exe"]).status();
    }
    let tmp = std::env::temp_dir().join("vesper-setup-payload.exe");
    std::fs::write(&tmp, PAYLOAD).map_err(|e| format!("Could not unpack the installer: {e}"))?;

    stage(app, "install");
    let mut c = quiet(Command::new(&tmp));
    c.arg("/S"); // silent, per-user: no admin prompt
    let status = c.status().map_err(|e| format!("Could not start the installer: {e}"));
    let _ = std::fs::remove_file(&tmp);
    let status = status?;
    if !status.success() {
        return Err(format!("The installer stopped (code {}).", status.code().map_or("unknown".into(), |c| c.to_string())));
    }

    stage(app, "shortcuts");
    let exe = installed_exe().ok_or("Vesper installed, but I could not find it afterwards.")?;
    if let Some(start) = dirs::data_dir().map(|d| d.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Vesper.lnk")) {
        if !start.exists() {
            let _ = make_shortcut(&start, &exe);
        }
    }
    if let Some(lnk) = dirs::desktop_dir().map(|d| d.join("Vesper.lnk")) {
        if desktop {
            if !lnk.exists() {
                let _ = make_shortcut(&lnk, &exe);
            }
        } else {
            let _ = std::fs::remove_file(&lnk);
        }
    }
    Ok(exe.to_string_lossy().to_string())
}

#[tauri::command]
async fn install(app: AppHandle, desktop: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || run_install(&app, desktop)).await.map_err(|e| e.to_string())?
}

fn run_uninstall(app: &AppHandle, remove_data: bool) -> Result<(), String> {
    let exe = installed_exe().ok_or("Vesper does not seem to be installed.")?;
    let dir = exe.parent().ok_or("Could not find the install folder.")?.to_path_buf();

    stage(app, "close");
    if let Some(name) = exe.file_name() {
        let _ = quiet(Command::new("taskkill")).args(["/F", "/IM"]).arg(name).status();
    }

    stage(app, "remove");
    let un = dir.join("uninstall.exe");
    if un.exists() {
        let mut c = quiet(Command::new(&un));
        c.arg("/S"); // silent; keeps your instances and worlds (they live in AppData\Roaming\Vesper)
        c.spawn().map_err(|e| format!("Could not start the uninstaller: {e}"))?;
        // NSIS uninstallers hand over to a temporary copy, so wait until the app is really gone.
        for _ in 0..120 {
            if !exe.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if exe.exists() {
            return Err("The uninstaller did not finish. Try again, or remove Vesper in Windows Settings > Apps.".into());
        }
    } else {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Could not remove Vesper: {e}"))?;
    }

    // Shortcuts (the uninstaller normally removes them; this catches the ones we made).
    if let Some(d) = dirs::desktop_dir() {
        let _ = std::fs::remove_file(d.join("Vesper.lnk"));
    }
    if let Some(d) = dirs::data_dir() {
        let _ = std::fs::remove_file(d.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Vesper.lnk"));
        if remove_data {
            let _ = std::fs::remove_dir_all(d.join("Vesper")); // instances, worlds, encrypted accounts
        }
    }
    Ok(())
}

#[tauri::command]
async fn uninstall(app: AppHandle, remove_data: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || run_uninstall(&app, remove_data)).await.map_err(|e| e.to_string())?
}

/// Path of an existing Vesper install, so the UI can say "Reinstall".
#[tauri::command]
fn installed() -> Option<String> {
    installed_exe().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn finish(app: AppHandle, launch: bool, exe: String) {
    if launch && !exe.is_empty() {
        let mut c = Command::new(&exe);
        if let Some(dir) = Path::new(&exe).parent() {
            c.current_dir(dir);
        }
        let _ = c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![install, installed, uninstall, finish])
        .run(tauri::generate_context!())
        .expect("Vesper Setup failed to start");
}
