//! "Rust" instances: Minecraft rewritten in Rust (the open-source Horizon project, MIT license,
//! github.com/horizon-rust/horizon), run instead of Java.
//!
//! Vesper does not bundle Horizon or any Minecraft file. On first Play it:
//!  1. downloads Vesper's rebranded build (or, until one exists, Horizon's official `nightly`) and checks its Ed25519-signed SHA-256,
//!  2. fetches the player's own Minecraft files from Mojang (only for a signed-in owner),
//!  3. runs `horizon-prepare`, which builds the game's resources from those files,
//!  4. starts `horizon-client`, handing over the Microsoft session on stdin (Horizon's Launch API).
use crate::{auth::Session, download::*, instances::{self, now, Instance}, launch::{self, Running}, mojang, paths};
use anyhow::{anyhow, bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, process::Stdio};
use tauri::{AppHandle, Emitter};
use tokio::{io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::Command};

/// The Minecraft version Horizon is built for. Updated from the build itself on first Play.
pub const MC_VERSION: &str = "26.3";

/// Vesper's own build of the Rust client (rebranded; made by the `vesper-rust` repository's workflow).
const VESPER_BASE: &str = "https://github.com/Vesper-CO/vesper-rust/releases/latest/download";
const VESPER_ARCHIVE: &str = "vesper-rust-windows-x64.zip";
/// Raw Ed25519 public key, BASE64 (32 bytes -> 44 characters ending in `=`), one line, no colons or spaces.
/// while it is empty, Vesper only uses the official Horizon build below.
const VESPER_KEY: &str = "3Hjopxi2FoT1ipLAAfNoN91ZGKY3uRrpicNU3YEUwPs=";
/// The official Horizon build, used until a Vesper build exists.
const HORIZON_BASE: &str = "https://github.com/horizon-rust/horizon/releases/download/nightly";
const API_RELEASE: &str = "https://api.github.com/repos/horizon-rust/horizon/releases/tags/nightly";
/// Horizon's release-signing key (crates/horizon-release-sign/release-keys.txt in its repository).
const HORIZON_KEY: &str = "MU4oiIQcCxkZvbwSjjXs7SySCuM6ywRQ3nGaJYZG0gw=";
/// How often Play looks for a newer Vesper build (two tiny files; the zip is only downloaded when it changed).
const RECHECK_SECS: u64 = 10 * 60;

struct Source {
    base: &'static str,
    archive: &'static str,
    vesper: bool,
}
fn sources() -> Vec<Source> {
    let mut v = vec![];
    if !VESPER_KEY.is_empty() && cfg!(not(target_arch = "aarch64")) {
        v.push(Source { base: VESPER_BASE, archive: VESPER_ARCHIVE, vesper: true });
    }
    let archive = if cfg!(target_arch = "aarch64") { "horizon-windows-arm64.zip" } else { "horizon-windows-x64.zip" };
    v.push(Source { base: HORIZON_BASE, archive, vesper: false });
    v
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    build: String,
    checked: u64,
    /// True when the installed build is Vesper's own (not the Horizon fallback).
    #[serde(default)]
    vesper: bool,
}
fn state_path() -> PathBuf { paths::rust().join("state.json") }
fn load_state() -> State {
    std::fs::read(state_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save_state(s: &State) {
    let _ = std::fs::create_dir_all(paths::rust());
    if let Ok(b) = serde_json::to_vec_pretty(s) {
        let _ = std::fs::write(state_path(), b);
    }
}

pub struct Build {
    pub id: String,
    pub dir: PathBuf,
}
fn build_dir(id: &str) -> PathBuf { paths::rust().join("builds").join(id) }
/// Vesper's build names its program vesper-client.exe; the official one is horizon-client.exe.
fn client_exe(dir: &Path) -> PathBuf {
    let v = dir.join("vesper-client.exe");
    if v.exists() { v } else { dir.join("horizon-client.exe") }
}
fn prepare_exe(dir: &Path) -> PathBuf { dir.join("horizon-prepare.exe") }

/// The native mods compiled into the installed Vesper build, as the build workflow listed them in
/// `vesper-mods.json` beside the client (id, name, version, summary, icon). An older build has no
/// such file, so none.
pub fn builtin_mods() -> Vec<serde_json::Value> {
    let dir = build_dir(&load_state().build);
    std::fs::read(dir.join("vesper-mods.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("mods").and_then(|m| m.as_array().cloned()))
        .unwrap_or_default()
}

/// Ids of those mods.
pub fn builtin_ids() -> Vec<String> {
    builtin_mods().iter().filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from)).collect()
}

/// Whether the installed client shows its own Discord presence (singleplayer, multiplayer, dimension...).
/// The build workflow records it in `vesper-mods.json` (`"discord": true`) when it was built with
/// Vesper's Discord application id. Older builds have no such file, so no.
fn client_has_presence() -> bool {
    let dir = build_dir(&load_state().build);
    std::fs::read(dir.join("vesper-mods.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("discord").and_then(|d| d.as_bool()))
        .unwrap_or(false)
}

/// The game shows its own Discord presence, so the launcher steps aside while it runs.
pub fn game_shows_presence() -> bool {
    crate::presence::available() && crate::settings::load().discord_presence && client_has_presence()
}

fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    // Vesper does not send anything to Horizon's servers: no "playing now" ping, no cosmetics
    // service. The client's own Discord presence runs only under Vesper's application, when the
    // build has one and the player has not turned it off in Vesper's settings.
    c.env("HORIZON_TELEMETRY_URL", "off").env("HORIZON_NO_HEARTBEAT", "1").env("HORIZON_COSMETICS", "0");
    if !game_shows_presence() {
        c.env("HORIZON_NO_DISCORD", "1");
    }
    #[cfg(windows)]
    c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    c.kill_on_drop(true);
    c
}

fn stage(app: &AppHandle, id: &str, s: &str) {
    let _ = app.emit("launch-stage", json!({"id": id, "stage": s}));
}

// ---------- finding and verifying the build ----------

/// "Nightly 2026-10-28 · 0c02067" -> "nightly-2026-10-28-0c02067" (what the v2 signature covers).
fn title_version(title: &str) -> Option<String> {
    let (date, commit) = title.strip_prefix("Nightly ")?.split_once(" · ")?;
    let date_ok = date.len() == 10 && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    let commit_ok = (7..=40).contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    (date_ok && commit_ok).then(|| format!("nightly-{date}-{commit}"))
}

fn signed(key: &VerifyingKey, message: &str, sums: &[u8], signatures: &str) -> bool {
    let mut msg = message.as_bytes().to_vec();
    msg.extend_from_slice(sums);
    signatures
        .lines()
        .filter_map(|l| B64.decode(l.trim()).ok())
        .filter_map(|b| Signature::from_slice(&b).ok())
        .any(|s| key.verify(&msg, &s).is_ok())
}

fn key_from(b64: &str) -> Result<VerifyingKey> {
    let raw = B64.decode(b64)?;
    Ok(VerifyingKey::from_bytes(&raw.try_into().map_err(|_| anyhow!("Bad built-in release key"))?)?)
}

/// Vesper's own builds: one signature over a fixed channel line and the checksums.
fn verify_vesper(sums: &[u8], sig: Option<&str>) -> Result<()> {
    let key = key_from(VESPER_KEY)?;
    if let Some(sig) = sig {
        if signed(&key, "vesper rust release\n", sums, sig) {
            return Ok(());
        }
    }
    bail!("The Vesper Rust build's signature did not match, so it was not installed.")
}

fn verify_horizon(sums: &[u8], v1: Option<&str>, v2: Option<&str>, title: Option<&str>) -> Result<()> {
    let key = key_from(HORIZON_KEY)?;
    if let (Some(sig), Some(version)) = (v2, title.and_then(title_version)) {
        if signed(&key, &format!("horizon release {version}\n"), sums, sig) {
            return Ok(());
        }
    }
    // Older channel signature, which Horizon still publishes beside the per-build one.
    if let Some(sig) = v1 {
        if signed(&key, "horizon release nightly\n", sums, sig) {
            return Ok(());
        }
    }
    bail!("The Rust build's signature did not match Horizon's release key, so it was not installed. Try again later.")
}

fn digest_for(sums: &[u8], name: &str) -> Result<String> {
    let text = std::str::from_utf8(sums).context("SHA256SUMS is not text")?;
    for line in text.lines() {
        if let Some((hex, file)) = line.split_once(' ') {
            if file.trim_start_matches([' ', '*']) == name && hex.len() == 64 {
                return Ok(hex.to_ascii_lowercase());
            }
        }
    }
    bail!("The release has no {name}")
}

async fn http_bytes(url: &str) -> Result<Vec<u8>> {
    Ok(client().get(url).send().await?.error_for_status()?.bytes().await?.to_vec())
}

async fn sha256_file(path: &Path) -> Result<String> {
    let p = path.to_owned();
    tokio::task::spawn_blocking(move || -> Result<String> {
        use std::io::Read;
        let mut f = std::fs::File::open(p)?;
        let mut h = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 { break; }
            h.update(&buf[..n]);
        }
        Ok(hex::encode(h.finalize()))
    })
    .await?
}

/// Unpack the archive, dropping its single top-level folder.
fn extract(zip_path: &Path, dest: &Path) -> Result<()> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        let Some(name) = e.enclosed_name() else { continue };
        let mut parts = name.components();
        parts.next();
        let rel: PathBuf = parts.collect();
        if rel.as_os_str().is_empty() { continue; }
        let out = dest.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(p) = out.parent() { std::fs::create_dir_all(p)?; }
        std::io::copy(&mut e, &mut std::fs::File::create(&out)?)?;
    }
    Ok(())
}

async fn fetch_from(app: &AppHandle, src: &Source) -> Result<(String, bool)> {
    let (base, name) = (src.base, src.archive);
    let sums = http_bytes(&format!("{base}/SHA256SUMS")).await.context("Could not reach the Rust build's releases")?;
    let sig = http_bytes(&format!("{base}/SHA256SUMS.sig")).await.ok().and_then(|b| String::from_utf8(b).ok());
    if src.vesper {
        verify_vesper(&sums, sig.as_deref())?;
    } else {
        let v2 = http_bytes(&format!("{base}/SHA256SUMS.v2.sig")).await.ok().and_then(|b| String::from_utf8(b).ok());
        let title = get_json::<Value>(API_RELEASE).await.ok().and_then(|v| v["name"].as_str().map(String::from));
        verify_horizon(&sums, sig.as_deref(), v2.as_deref(), title.as_deref())?;
    }

    let digest = digest_for(&sums, name)?;
    let id = digest[..12].to_string();
    let dir = build_dir(&id);
    if client_exe(&dir).exists() {
        return Ok((id, src.vesper));
    }
    let zip = paths::rust().join("tmp").join(name);
    let _ = tokio::fs::remove_file(&zip).await;
    let _ = tokio::fs::remove_file(format!("{}.part", zip.display())).await;
    download_all(app, "Downloading the Rust build", vec![Job { url: format!("{base}/{name}"), path: zip.clone(), sha1: None, size: None }]).await?;
    if sha256_file(&zip).await? != digest {
        let _ = tokio::fs::remove_file(&zip).await;
        bail!("The downloaded Rust build did not match its signed checksum, so it was not installed.");
    }
    let partial = paths::rust().join("builds").join(format!("{id}.partial"));
    let (z, p) = (zip.clone(), partial.clone());
    tokio::task::spawn_blocking(move || extract(&z, &p)).await??;
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&partial, &dir)?;
    let _ = std::fs::remove_file(&zip);
    Ok((id, src.vesper))
}

/// Try Vesper's own build first, then the official one.
async fn fetch_latest(app: &AppHandle) -> Result<(String, bool)> {
    let mut last = anyhow!("No Rust build source is available");
    for src in sources() {
        match fetch_from(app, &src).await {
            Ok(r) => return Ok(r),
            Err(e) => last = e,
        }
    }
    Err(last)
}

/// Keep the current build and the previous one; remove the rest (and their prepared files).
fn prune(current: &str) {
    let Ok(rd) = std::fs::read_dir(paths::rust().join("builds")) else { return };
    let mut others: Vec<(std::time::SystemTime, String)> = rd
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            (n != current && !n.ends_with(".partial")).then(|| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), n))
        })
        .collect();
    others.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, n) in others.into_iter().skip(1) {
        let _ = std::fs::remove_dir_all(build_dir(&n));
        let _ = std::fs::remove_dir_all(paths::rust().join("prepared").join(&n));
    }
}

pub async fn ensure_build(app: &AppHandle) -> Result<Build> {
    let st = load_state();
    let have = (!st.build.is_empty() && client_exe(&build_dir(&st.build)).exists()).then(|| st.build.clone());
    if let Some(id) = &have {
        if now().saturating_sub(st.checked) < RECHECK_SECS {
            return Ok(Build { id: id.clone(), dir: build_dir(id) });
        }
    }
    match fetch_latest(app).await {
        Ok((id, vesper)) => {
            save_state(&State { build: id.clone(), checked: now(), vesper });
            prune(&id);
            Ok(Build { dir: build_dir(&id), id })
        }
        // Offline, or GitHub is down: keep playing on the build we already have.
        Err(_) if have.is_some() => {
            let id = have.unwrap();
            Ok(Build { dir: build_dir(&id), id })
        }
        Err(e) => Err(e),
    }
}

// ---------- preparing the game's resources from the player's own Minecraft ----------

async fn launch_info(build: &Build) -> Result<Value> {
    let out = command(&prepare_exe(&build.dir)).arg("--launch-api").current_dir(&build.dir).output().await.context("Could not run horizon-prepare")?;
    if !out.status.success() {
        bail!("horizon-prepare: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

async fn ensure_prepared(app: &AppHandle, id: &str, build: &Build) -> Result<(PathBuf, String)> {
    let info = launch_info(build).await?;
    let mc = info["minecraft"].as_str().unwrap_or(MC_VERSION).to_string();
    let prepared = paths::rust().join("prepared").join(&build.id);

    let fresh = command(&prepare_exe(&build.dir)).arg("--check").arg("--output").arg(&prepared).current_dir(&build.dir).output().await.map(|o| o.status.success()).unwrap_or(false);
    if fresh {
        return Ok((prepared, mc));
    }

    stage(app, id, "Getting your Minecraft files");
    let v = mojang::resolve(&mc).await.with_context(|| format!("Could not get Minecraft {mc} from Mojang"))?;
    let game_dir = paths::game_dir(id);
    mojang::ensure_game(app, &v).await?;
    mojang::ensure_assets(app, &v, &game_dir).await?;
    let java = mojang::ensure_runtime(app, &v).await?;
    let java_home = java.parent().and_then(|p| p.parent()).ok_or_else(|| anyhow!("Unexpected Java folder"))?.to_path_buf();

    let s = &v["downloads"]["server"];
    let jar = paths::rust().join("server").join(&mc).join("server.jar");
    download_all(
        app,
        "Minecraft server file",
        vec![Job { url: s["url"].as_str().ok_or_else(|| anyhow!("Mojang lists no server file for {mc}"))?.into(), path: jar.clone(), sha1: s["sha1"].as_str().map(String::from), size: s["size"].as_u64() }],
    )
    .await?;

    stage(app, id, "Preparing Rust resources (a few minutes, first time only)");
    let mut c = command(&prepare_exe(&build.dir));
    c.current_dir(&build.dir)
        .args(["--prepare", "--server-jar"]).arg(&jar)
        .arg("--minecraft-dir").arg(paths::shared())
        .arg("--output").arg(&prepared)
        .args(["--complete", "--progress", "json", "--java-home"]).arg(&java_home)
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().context("Could not start horizon-prepare")?;
    let mut stderr = child.stderr.take();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() { let _ = e.read_to_string(&mut s).await; }
        s
    });
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut error = None;
    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if let Some(m) = v["message"].as_str() { launch::log(app, id, m, "out"); }
        if let Some(f) = v["fraction"].as_f64() {
            let _ = app.emit("progress", json!({"label": "Preparing Rust resources", "done": (f.clamp(0.0, 1.0) * 1000.0) as u64, "total": 1000}));
        }
        if let Some(e) = v["error"].as_str() { error = Some(e.to_string()); }
    }
    let status = child.wait().await?;
    let tail = err_task.await.unwrap_or_default();
    if !status.success() {
        let tail: Vec<&str> = tail.lines().rev().take(4).collect();
        bail!("Preparing Rust resources failed: {}", error.unwrap_or_else(|| tail.into_iter().rev().collect::<Vec<_>>().join(" ")));
    }
    Ok((prepared, mc))
}

// ---------- starting the game ----------

pub async fn play(app: AppHandle, running: &Running, mut inst: Instance, sess: Session) -> Result<()> {
    let id = inst.id.clone();
    stage(&app, &id, "Checking the Rust build");
    let build = ensure_build(&app).await?;
    let (prepared, mc) = ensure_prepared(&app, &id, &build).await?;
    if inst.mc_version != mc {
        inst.mc_version = mc;
    }

    let game = paths::game_dir(&id);
    let home = paths::instance_dir(&id).join("home");
    std::fs::create_dir_all(&game)?;
    std::fs::create_dir_all(&home)?;
    // The native mods built into the client start switched off; the player turns them on in the game.
    let _ = crate::horizon_mods::start_builtin_off(&id);

    stage(&app, &id, "Launching");
    let mut c = command(&client_exe(&build.dir));
    c.current_dir(&game)
        .arg("--resources").arg(&prepared)
        .arg("--game-dir").arg(&game)
        .arg("--home").arg(&home)
        .arg("--session-stdin")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().with_context(|| format!("Could not start {}", client_exe(&build.dir).display()))?;

    // The session goes in on standard input, never on the command line.
    let session = json!({"name": sess.name, "uuid": sess.uuid, "access_token": sess.token, "user_type": "msa"}).to_string();
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(session.as_bytes()).await?;
        let _ = stdin.shutdown().await;
    }

    inst.last_played = now();
    let _ = instances::save(&inst);
    launch::supervise(&app, running, id.clone(), child);
    stage(&app, &id, "Running");
    Ok(())
}
