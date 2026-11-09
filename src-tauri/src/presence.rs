//! Discord Rich Presence for the launcher: "In the launcher: Browsing mods" and "Playing <instance>".
//!
//! It talks to the Discord app on this computer through Discord's local IPC (a named pipe
//! `discord-ipc-N` on Windows, a socket on Linux and macOS): frames of an opcode (u32 LE), a length
//! (u32 LE) and JSON. A thread of its own does the talking, so the launcher never waits for Discord:
//! it connects when Discord runs, tries again every 15 seconds when it does not, stays under Discord's
//! update limit, and drops the connection (which clears the activity) when there is nothing to show.
//!
//! The application id is Vesper's application on Discord's developer portal. It is public, and set
//! when the launcher is built: `VESPER_DISCORD_APP_ID` (the release workflow takes it from the
//! repository variable DISCORD_APP_ID). Without it presence is off and nothing is sent. The art asset
//! it uses is called `vesper`. Only the instance name and its game version are ever shown, never
//! servers, accounts or worlds.
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const APP_ID: &str = match option_env!("VESPER_DISCORD_APP_ID") {
    Some(id) => id,
    None => "",
};
const DOWNLOAD: &str = "https://github.com/Vesper-CO/vesper/releases/latest";
/// Discord drops updates beyond 5 in 20 seconds.
const GAP: Duration = Duration::from_secs(4);
const RETRY: Duration = Duration::from_secs(15);
/// The activity is sent again this often, so a Discord that restarted gets it back.
const REFRESH: Duration = Duration::from_secs(60);
/// Points the launcher at a test's fake Discord (a folder holding `discord-ipc-0`).
const IPC_DIR_ENV: &str = "VESPER_DISCORD_IPC_DIR";

pub fn valid_id(id: &str) -> bool {
    (17..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit())
}
/// Whether this build of Vesper has a Discord application to show presence under.
pub fn available() -> bool { valid_id(APP_ID) }

fn now_secs() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

// ---------- what to show ----------

#[derive(Clone, Debug, PartialEq)]
struct Playing {
    title: String,
    detail: String,
    since: u64,
    /// The game shows its own presence (Vesper's Rust client does), so the launcher steps aside.
    game_owns: bool,
}

#[derive(Debug)]
struct Model {
    enabled: bool,
    page: String,
    launched: u64,
    playing: Option<Playing>,
}

enum Msg {
    Enabled(bool),
    Page(String),
    Playing(Option<Playing>),
}

fn clip(s: &str, max: usize) -> String {
    let clean: String = s.chars().filter(|c| !c.is_control()).collect();
    let clean = clean.trim();
    if clean.chars().count() <= max { clean.to_string() } else { format!("{}…", clean.chars().take(max - 1).collect::<String>()) }
}

fn activity(details: &str, state: &str, since: u64) -> Value {
    json!({
        "details": clip(details, 100),
        "state": clip(state, 100),
        "timestamps": { "start": since },
        "assets": { "large_image": "vesper", "large_text": "Vesper" },
        "buttons": [{ "label": "Get Vesper", "url": DOWNLOAD }],
    })
}

impl Model {
    /// The activity to show now, or None for nothing (then the connection is dropped).
    fn activity(&self) -> Option<Value> {
        if !self.enabled {
            return None;
        }
        match &self.playing {
            Some(p) if p.game_owns => None,
            Some(p) => Some(activity(&format!("Playing {}", p.title), &p.detail, p.since)),
            None => Some(activity("In the launcher", &self.page, self.launched)),
        }
    }
    fn apply(&mut self, m: Msg) {
        match m {
            Msg::Enabled(on) => self.enabled = on,
            Msg::Page(p) => self.page = p,
            Msg::Playing(p) => self.playing = p,
        }
    }
}

pub fn page_label(page: &str) -> &'static str {
    match page {
        "home" => "On the home page",
        "instances" => "Looking at instances",
        "mods" => "Browsing mods",
        "accounts" => "Managing accounts",
        "settings" => "In the settings",
        _ => "Using the launcher",
    }
}

/// "Minecraft 1.21.1 · Fabric"
pub fn detail(mc_version: &str, loader: &str) -> String {
    let loader = match loader {
        "vanilla" => "Vanilla",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "rust" => "Rust",
        other => other,
    };
    format!("Minecraft {mc_version} · {loader}")
}

// ---------- Discord's IPC ----------

trait Rw: Read + Write + Send {}
impl<T: Read + Write + Send> Rw for T {}

struct Conn {
    io: Box<dyn Rw>,
    nonce: u64,
}

#[cfg(windows)]
fn connect(i: usize) -> io::Result<Box<dyn Rw>> {
    let f = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\?\pipe\discord-ipc-{i}"))?;
    Ok(Box::new(f))
}

#[cfg(unix)]
fn connect(i: usize) -> io::Result<Box<dyn Rw>> {
    use std::os::unix::net::UnixStream;
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(d) = std::env::var_os(IPC_DIR_ENV) {
        dirs.push(d.into());
    } else {
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            if let Some(d) = std::env::var_os(key) {
                dirs.push(d.into());
            }
        }
        dirs.push("/tmp".into());
    }
    for d in dirs {
        if let Ok(s) = UnixStream::connect(d.join(format!("discord-ipc-{i}"))) {
            return Ok(Box::new(s));
        }
    }
    Err(io::ErrorKind::NotFound.into())
}

fn write_frame(io: &mut dyn Rw, op: u32, body: &Value) -> io::Result<()> {
    let data = serde_json::to_vec(body)?;
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&data);
    io.write_all(&buf)?;
    io.flush()
}

fn read_frame(io: &mut dyn Rw) -> io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    io.read_exact(&mut head)?;
    let op = u32::from_le_bytes(head[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
    if len > 1 << 16 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Discord sent an oversized frame"));
    }
    let mut data = vec![0u8; len];
    io.read_exact(&mut data)?;
    Ok((op, serde_json::from_slice(&data).unwrap_or(Value::Null)))
}

impl Conn {
    fn open(app_id: &str) -> io::Result<Conn> {
        let mut last = io::Error::from(io::ErrorKind::NotFound);
        for i in 0..10 {
            let mut io = match connect(i) {
                Ok(io) => io,
                Err(e) => { last = e; continue; }
            };
            if write_frame(&mut *io, 0, &json!({ "v": 1, "client_id": app_id })).is_err() {
                continue;
            }
            match read_frame(&mut *io) {
                Ok((1, v)) if v["evt"] == "READY" => return Ok(Conn { io, nonce: 0 }),
                Ok((_, v)) => last = io::Error::new(io::ErrorKind::Other, format!("Discord refused: {}", v["data"]["message"])),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn set(&mut self, activity: &Value) -> io::Result<()> {
        self.nonce += 1;
        let body = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": self.nonce.to_string(),
        });
        write_frame(&mut *self.io, 1, &body)?;
        loop {
            match read_frame(&mut *self.io)? {
                (3, v) => write_frame(&mut *self.io, 4, &v)?, // ping -> pong
                (2, _) => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Discord closed the connection")),
                (_, v) if v["evt"] == "ERROR" => return Err(io::Error::new(io::ErrorKind::Other, v["data"]["message"].to_string())),
                _ => return Ok(()),
            }
        }
    }
}

// ---------- the thread ----------

fn run(rx: mpsc::Receiver<Msg>, app_id: String, mut model: Model) {
    let long_ago = |now: Instant| now.checked_sub(Duration::from_secs(3600)).unwrap_or(now);
    let mut conn: Option<Conn> = None;
    let mut sent: Option<Value> = None;
    let mut last_send = long_ago(Instant::now());
    let mut next_try = Instant::now();
    loop {
        let want = model.activity();
        let now = Instant::now();
        match &want {
            None => {
                // Nothing to show: closing the connection clears the activity.
                conn = None;
                sent = None;
            }
            Some(w) => {
                if conn.is_none() && now >= next_try {
                    match Conn::open(&app_id) {
                        Ok(c) => { conn = Some(c); sent = None; }
                        Err(_) => next_try = now + RETRY,
                    }
                }
                let due = sent.as_ref() != Some(w) || now.duration_since(last_send) >= REFRESH;
                if let Some(c) = conn.as_mut() {
                    if due && now.duration_since(last_send) >= GAP {
                        match c.set(w) {
                            Ok(()) => { sent = Some(w.clone()); last_send = now; }
                            Err(_) => { conn = None; next_try = now + RETRY; }
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        let wait = match &want {
            None => Duration::from_secs(3600),
            Some(_) if conn.is_none() => next_try.saturating_duration_since(now),
            Some(w) if sent.as_ref() != Some(w) => (last_send + GAP).saturating_duration_since(now),
            Some(_) => (last_send + REFRESH).saturating_duration_since(now),
        };
        match rx.recv_timeout(wait) {
            Ok(m) => model.apply(m),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(m) = rx.try_recv() {
            model.apply(m);
        }
    }
}

// ---------- what the launcher calls ----------

static TX: OnceLock<Mutex<mpsc::Sender<Msg>>> = OnceLock::new();
static RUNNING: AtomicUsize = AtomicUsize::new(0);

fn send(m: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.lock().unwrap().send(m);
    }
}

/// Starts the presence thread (once). Does nothing in a build without an application id.
pub fn start(enabled: bool) {
    if !available() || TX.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    if TX.set(Mutex::new(tx)).is_err() {
        return;
    }
    let model = Model { enabled, page: page_label("home").into(), launched: now_secs(), playing: None };
    let _ = std::thread::Builder::new().name("discord-presence".into()).spawn(move || run(rx, APP_ID.to_string(), model));
}

pub fn set_enabled(on: bool) { send(Msg::Enabled(on)) }
pub fn set_page(page: &str) { send(Msg::Page(page_label(page).into())) }

/// An instance started. `game_owns`: the game shows its own presence, so the launcher steps aside.
pub fn playing(title: &str, detail: &str, game_owns: bool) {
    RUNNING.fetch_add(1, Ordering::SeqCst);
    send(Msg::Playing(Some(Playing { title: title.into(), detail: detail.into(), since: now_secs(), game_owns })));
}

/// An instance ended: back to the launcher once none is left.
pub fn stopped() {
    let left = RUNNING.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1))).unwrap_or(1).saturating_sub(1);
    if left == 0 {
        send(Msg::Playing(None));
    }
}

#[tauri::command]
pub fn presence_available() -> bool { available() }

#[tauri::command]
pub fn presence_page(page: String) { set_page(&page) }

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn model_decides_what_to_show() {
        let mut m = Model { enabled: true, page: "Browsing mods".into(), launched: 10, playing: None };
        let a = m.activity().unwrap();
        assert_eq!(a["details"], "In the launcher");
        assert_eq!(a["state"], "Browsing mods");
        assert_eq!(a["timestamps"]["start"], 10);
        assert_eq!(a["assets"]["large_image"], "vesper");
        m.apply(Msg::Playing(Some(Playing { title: "My Fabric".into(), detail: detail("1.21.1", "fabric"), since: 99, game_owns: false })));
        let a = m.activity().unwrap();
        assert_eq!(a["details"], "Playing My Fabric");
        assert_eq!(a["state"], "Minecraft 1.21.1 · Fabric");
        assert_eq!(a["timestamps"]["start"], 99);
        m.apply(Msg::Playing(Some(Playing { title: "R".into(), detail: detail("26.3", "rust"), since: 1, game_owns: true })));
        assert!(m.activity().is_none(), "the game shows its own presence");
        m.apply(Msg::Playing(None));
        m.apply(Msg::Enabled(false));
        assert!(m.activity().is_none(), "off shows nothing");
        assert_eq!(clip("a\nb\u{7}c", 10), "abc");
        assert_eq!(clip(&"x".repeat(300), 100).chars().count(), 100);
        assert!(valid_id("1563857609738690560") && !valid_id("abc") && !valid_id(""));
    }

    #[test]
    fn talks_to_a_fake_discord() {
        let dir = std::env::temp_dir().join(format!("vesper-ipc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("discord-ipc-0"));
        let listener = UnixListener::bind(dir.join("discord-ipc-0")).unwrap();
        // The test does not read the environment of other tests: connect() reads it once per call.
        unsafe { std::env::set_var(IPC_DIR_ENV, &dir) };

        let server = std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut s: Box<dyn Rw> = Box::new(s);
            let (op, hello) = read_frame(&mut *s).unwrap();
            assert_eq!(op, 0);
            assert_eq!(hello["client_id"], "1234567890123456789");
            write_frame(&mut *s, 1, &json!({ "cmd": "DISPATCH", "evt": "READY", "data": {} })).unwrap();
            let (op, set) = read_frame(&mut *s).unwrap();
            assert_eq!(op, 1);
            assert_eq!(set["cmd"], "SET_ACTIVITY");
            assert_eq!(set["args"]["activity"]["details"], "In the launcher");
            assert_eq!(set["args"]["activity"]["buttons"][0]["label"], "Get Vesper");
            write_frame(&mut *s, 1, &json!({ "cmd": "SET_ACTIVITY", "nonce": set["nonce"] })).unwrap();
            // The launcher then closes the connection when asked to show nothing.
            let mut rest = Vec::new();
            let _ = s.read_to_end(&mut rest);
        });

        let (tx, rx) = mpsc::channel();
        let model = Model { enabled: true, page: "On the home page".into(), launched: now_secs(), playing: None };
        let t = std::thread::spawn(move || run(rx, "1234567890123456789".into(), model));
        std::thread::sleep(Duration::from_millis(700));
        tx.send(Msg::Enabled(false)).unwrap();
        server.join().unwrap(); // returns when the connection closed
        drop(tx);
        t.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
