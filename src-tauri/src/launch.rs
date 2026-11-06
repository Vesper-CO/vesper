//! Builds the Java command line from Mojang's version JSON and runs the game,
//! streaming stdout/stderr to the UI console.
use crate::{auth, error::CmdResult, instances, loaders, mojang, paths};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::{collections::HashMap, io::Read, path::Path, process::Stdio, sync::Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::oneshot,
};

#[derive(Default)]
pub struct Running(pub Mutex<HashMap<String, oneshot::Sender<()>>>);

const SEP: &str = if cfg!(windows) { ";" } else { ":" };

fn stage(app: &AppHandle, id: &str, s: &str) {
    let _ = app.emit("launch-stage", json!({"id": id, "stage": s}));
}

fn sub(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut r = s.to_string();
    for (k, v) in vars {
        r = r.replace(&format!("${{{k}}}"), v);
    }
    r
}

fn expand(list: &Value, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = vec![];
    for a in list.as_array().into_iter().flatten() {
        match a {
            Value::String(s) => out.push(sub(s, vars)),
            Value::Object(_) if mojang::rules_ok(a.get("rules")) => match &a["value"] {
                Value::String(s) => out.push(sub(s, vars)),
                Value::Array(vs) => out.extend(vs.iter().filter_map(|x| x.as_str()).map(|s| sub(s, vars))),
                _ => {}
            },
            _ => {}
        }
    }
    out
}

fn extract_natives(jars: &[std::path::PathBuf], dir: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)?;
    for j in jars {
        let mut z = zip::ZipArchive::new(std::fs::File::open(j)?)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let Some(rel) = e.enclosed_name() else { continue };
            if e.is_dir() || rel.starts_with("META-INF") {
                continue;
            }
            let out = dir.join(rel.file_name().unwrap_or_default());
            let mut buf = vec![];
            e.read_to_end(&mut buf)?;
            std::fs::write(out, buf)?;
        }
    }
    Ok(())
}

fn pipe<R: AsyncRead + Unpin + Send + 'static>(app: AppHandle, id: String, r: R, stream: &'static str) {
    tokio::spawn(async move {
        let mut rd = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match rd.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim_end().to_string();
                    let _ = app.emit("game-log", json!({"id": id, "line": line, "stream": stream}));
                }
            }
        }
    });
}

#[tauri::command]
pub async fn play(app: AppHandle, state: State<'_, Running>, id: String) -> CmdResult<()> {
    if state.0.lock().unwrap().contains_key(&id) {
        return Err(anyhow!("This instance is already running").into());
    }
    let mut inst = instances::load(&id)?;

    stage(&app, &id, "Checking your account");
    let sess = auth::session().await?;

    stage(&app, &id, "Preparing version");
    if inst.version_id.is_empty() || !paths::version_json(&inst.version_id).exists() {
        loaders::install(&app, &mut inst).await?;
        instances::save(&inst)?;
    }
    let v = mojang::resolve(&inst.version_id).await?;
    let game_dir = paths::game_dir(&id);
    tokio::fs::create_dir_all(&game_dir).await?;

    stage(&app, &id, "Downloading game files");
    let libs = mojang::ensure_game(&app, &v).await?;
    stage(&app, &id, "Downloading assets");
    let assets = mojang::ensure_assets(&app, &v, &game_dir).await?;
    stage(&app, &id, "Preparing Java");
    let java = mojang::ensure_java(&app, &v).await?;

    // Natives: old versions need extraction; newer ones load them from the classpath jars.
    let natives_dir = paths::instance_dir(&id).join("natives");
    tokio::fs::create_dir_all(&natives_dir).await?;
    if !libs.natives.is_empty() {
        let (n, d) = (libs.natives.clone(), natives_dir.clone());
        tokio::task::spawn_blocking(move || extract_natives(&n, &d)).await??;
    }

    // Classpath. Forge/NeoForge expect `versions/<id>/<id>.jar` (ignored via -DignoreList).
    let mut cp: Vec<String> = libs.classpath.iter().map(|p| p.to_string_lossy().to_string()).collect();
    let jar = if inst.loader == "forge" || inst.loader == "neoforge" { paths::version_jar(&inst.version_id) } else { paths::version_jar(&mojang::jar_id(&v)) };
    cp.push(jar.to_string_lossy().to_string());

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", sess.name);
    vars.insert("auth_uuid", sess.uuid);
    vars.insert("auth_access_token", sess.token);
    vars.insert("auth_xuid", String::new());
    vars.insert("clientid", String::new());
    vars.insert("user_type", "msa".into());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_name", v["id"].as_str().unwrap_or(&inst.version_id).to_string());
    vars.insert("version_type", v["type"].as_str().unwrap_or("release").to_string());
    vars.insert("game_directory", game_dir.to_string_lossy().to_string());
    vars.insert("assets_root", paths::assets().to_string_lossy().to_string());
    vars.insert("assets_index_name", assets.index_id.clone());
    vars.insert("game_assets", assets.game_assets.to_string_lossy().to_string());
    vars.insert("natives_directory", natives_dir.to_string_lossy().to_string());
    vars.insert("library_directory", paths::libraries().to_string_lossy().to_string());
    vars.insert("classpath_separator", SEP.into());
    vars.insert("classpath", cp.join(SEP));
    vars.insert("launcher_name", "Vesper".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());

    let mut args: Vec<String> = vec![
        format!("-Xmx{}M", inst.ram_mb),
        format!("-Xms{}M", inst.ram_mb.min(512)),
        "-Dlog4j2.formatMsgNoLookups=true".into(), // Log4Shell mitigation for older versions
    ];
    args.extend(inst.jvm_args.split_whitespace().map(String::from));
    if v["arguments"]["jvm"].is_array() {
        args.extend(expand(&v["arguments"]["jvm"], &vars));
    } else {
        args.push(format!("-Djava.library.path={}", natives_dir.display()));
        args.push("-cp".into());
        args.push(cp.join(SEP));
    }
    args.push(v["mainClass"].as_str().ok_or_else(|| anyhow!("Version has no mainClass"))?.to_string());
    if v["arguments"]["game"].is_array() {
        args.extend(expand(&v["arguments"]["game"], &vars));
    } else if let Some(legacy) = v["minecraftArguments"].as_str() {
        args.extend(legacy.split_whitespace().map(|s| sub(s, &vars)));
    }

    stage(&app, &id, "Launching");
    let mut cmd = Command::new(&java);
    cmd.args(&args).current_dir(&game_dir).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no stray console
    let mut child = cmd.spawn().with_context(|| format!("Could not start Java at {}", java.display()))?;

    pipe(app.clone(), id.clone(), child.stdout.take().unwrap(), "out");
    pipe(app.clone(), id.clone(), child.stderr.take().unwrap(), "err");
    let (tx, rx) = oneshot::channel::<()>();
    state.0.lock().unwrap().insert(id.clone(), tx);

    inst.last_played = instances::now();
    let _ = instances::save(&inst);

    let (app2, id2) = (app.clone(), id.clone());
    tokio::spawn(async move {
        let code = tokio::select! {
            s = child.wait() => s.ok().and_then(|s| s.code()),
            _ = rx => { let _ = child.kill().await; None }
        };
        app2.state::<Running>().0.lock().unwrap().remove(&id2);
        let _ = app2.emit("game-exit", json!({"id": id2, "code": code}));
    });
    stage(&app, &id, "Running");
    Ok(())
}

#[tauri::command]
pub fn kill_game(state: State<'_, Running>, id: String) {
    if let Some(tx) = state.0.lock().unwrap().remove(&id) {
        let _ = tx.send(());
    }
}

#[tauri::command]
pub fn running_instances(state: State<'_, Running>) -> Vec<String> {
    state.0.lock().unwrap().keys().cloned().collect()
}
