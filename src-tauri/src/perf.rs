//! Performance tuning for Java instances (everything except "Rust" instances, which have no JVM).
//!
//! Three things are decided here, all from plain numbers so they can be unit-tested without Tauri:
//!   * which JVM flags to add (garbage collector settings tuned for a game client, per profile),
//!   * how much memory to give the game (never so much that Windows starts swapping, which is the
//!     fastest way to wreck FPS), plus a sensible suggestion when the player has not chosen,
//!   * whether to run the game at a higher process priority.
//!
//! The player's own "Extra JVM arguments" are always added AFTER these, so they win.
use serde::Serialize;

pub const PROFILES: [&str; 4] = ["auto", "fps", "lowram", "off"];

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// The profile that was actually used ("auto" is resolved to one of the others).
    pub profile: String,
    pub xmx_mb: u32,
    pub xms_mb: u32,
    pub flags: Vec<String>,
    pub high_priority: bool,
    /// Human-readable explanations, shown in the game console and the instance editor.
    pub notes: Vec<String>,
}

// ---------- system memory ----------

#[cfg(windows)]
fn os_total_ram_mb() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(m: *mut MemoryStatusEx) -> i32;
    }
    let mut m = MemoryStatusEx { length: std::mem::size_of::<MemoryStatusEx>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virtual: 0, avail_virtual: 0, avail_extended: 0 };
    // SAFETY: `m` is a correctly sized, initialised MEMORYSTATUSEX with `length` set, as the API requires.
    let ok = unsafe { GlobalMemoryStatusEx(&mut m) };
    (ok != 0).then(|| m.total_phys / (1024 * 1024))
}

#[cfg(not(windows))]
fn os_total_ram_mb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb: u64 = s.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Installed RAM in MB (8 GB if the OS will not say).
pub fn total_ram_mb() -> u64 {
    os_total_ram_mb().filter(|&m| m >= 512).unwrap_or(8192)
}

// ---------- Java version guess ----------

/// Which Java Mojang ships for a Minecraft version. Used only for the editor's preview: when the
/// game really starts, the exact number comes from the version's own JSON.
pub fn java_major_for(mc: &str) -> u32 {
    let mut parts = mc.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse::<u32>().unwrap_or(0));
    let (a, b, c) = (parts.next().unwrap_or(1), parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    if a >= 26 { return 25; } // year-style versions (26.x)
    match (b, c) {
        (0..=16, _) => 8,
        (17, _) => 17,
        (18..=19, _) => 17,
        (20, 0..=4) => 17,
        _ => 21,
    }
}

// ---------- planning ----------

pub fn resolve_profile(profile: &str, total_mb: u64) -> &'static str {
    match profile {
        "fps" => "fps",
        "lowram" => "lowram",
        "off" => "off",
        // "auto": a PC with 8 GB or less has no memory to spare, so favour a small footprint.
        _ => if total_mb <= 8192 { "lowram" } else { "fps" },
    }
}

fn f(s: &str) -> String { s.to_string() }

pub fn plan(profile: &str, ram_mb: u32, java_major: u32, total_mb: u64) -> Plan {
    let prof = resolve_profile(profile, total_mb);
    let mut notes = vec![];

    if prof == "off" {
        return Plan { profile: f(prof), xmx_mb: ram_mb, xms_mb: ram_mb.min(512), flags: vec![], high_priority: false, notes: vec![f("Tuning is off: only your own memory and JVM settings are used.")] };
    }

    // Leave the OS (and the launcher) room: never hand the game more than 75% of the PC's memory.
    let cap = ((total_mb * 3 / 4).min(total_mb.saturating_sub(2048))).max(1024) as u32;
    let mut xmx = ram_mb.max(512);
    if xmx > cap {
        notes.push(format!("Memory lowered from {} MB to {} MB: this PC has {} MB, and going over starts swapping to disk.", xmx, cap, total_mb));
        xmx = cap;
    }
    let xms = match prof {
        "fps" => (xmx / 2).clamp(512, 2048), // a roomy starting heap means fewer resize pauses
        _ => 256,
    }
    .min(xmx);

    let mut flags = vec![
        f("-XX:+IgnoreUnrecognizedVMOptions"), // flags from another Java version are skipped, never fatal
        f("-XX:+UnlockExperimentalVMOptions"),
        f("-XX:+UseG1GC"),
        f("-XX:+ParallelRefProcEnabled"),
        f("-XX:+DisableExplicitGC"),   // some mods call System.gc(), which freezes the game
        f("-XX:+PerfDisableSharedMem"), // stops the JVM writing a perf file that can stall on Windows
    ];
    match prof {
        "fps" => {
            for x in ["-XX:MaxGCPauseMillis=30", "-XX:G1NewSizePercent=30", "-XX:G1MaxNewSizePercent=50", "-XX:G1HeapRegionSize=8M", "-XX:G1ReservePercent=20", "-XX:SurvivorRatio=32", "-XX:MaxTenuringThreshold=1"] {
                flags.push(f(x));
            }
            notes.push(f("Max FPS: short, frequent garbage collections to avoid frame stutters, and a higher process priority."));
        }
        _ => {
            for x in ["-XX:MaxGCPauseMillis=60", "-XX:MinHeapFreeRatio=10", "-XX:MaxHeapFreeRatio=30", "-XX:G1PeriodicGCInterval=30000", "-XX:+UseStringDeduplication"] {
                flags.push(f(x));
            }
            notes.push(f("Low memory: starts with a small heap, hands unused memory back to Windows, and removes duplicate strings."));
        }
    }
    if java_major >= 24 {
        flags.push(f("-XX:+UseCompactObjectHeaders")); // smaller objects: less memory and better cache use
        notes.push(f("Compact object headers are on (Java 24+), which shrinks the game's memory use."));
    }
    Plan { profile: f(prof), xmx_mb: xmx, xms_mb: xms, flags, high_priority: prof == "fps", notes }
}

/// A memory amount that fits what is installed: enough for the mod count, never more than half the PC.
pub fn recommend_ram_mb(total_mb: u64, loader: &str, mods: usize) -> u32 {
    let want: u32 = if loader == "vanilla" || mods <= 10 { 3072 } else if mods <= 40 { 4096 } else if mods <= 100 { 6144 } else if mods <= 200 { 8192 } else { 10240 };
    let cap = (total_mb / 2) as u32;
    (want.min(cap).max(1024) / 512) * 512
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_versions() {
        assert_eq!(java_major_for("1.8.9"), 8);
        assert_eq!(java_major_for("1.16.5"), 8);
        assert_eq!(java_major_for("1.18.2"), 17);
        assert_eq!(java_major_for("1.20.4"), 17);
        assert_eq!(java_major_for("1.20.6"), 21);
        assert_eq!(java_major_for("1.21.4"), 21);
        assert_eq!(java_major_for("26.3"), 25);
        assert_eq!(java_major_for("26.4-snapshot-8"), 25);
    }

    #[test]
    fn off_changes_nothing() {
        let p = plan("off", 4096, 21, 16384);
        assert!(p.flags.is_empty() && !p.high_priority);
        assert_eq!((p.xmx_mb, p.xms_mb), (4096, 512));
    }

    #[test]
    fn auto_follows_installed_memory() {
        assert_eq!(resolve_profile("auto", 8192), "lowram");
        assert_eq!(resolve_profile("auto", 16384), "fps");
        assert_eq!(resolve_profile("fps", 4096), "fps");
    }

    #[test]
    fn memory_is_capped_and_xms_never_exceeds_xmx() {
        let p = plan("fps", 12288, 21, 8192); // asks for 12 GB on an 8 GB PC
        assert_eq!(p.xmx_mb, 6144);
        assert!(p.xms_mb <= p.xmx_mb);
        assert!(p.notes.iter().any(|n| n.contains("lowered")));
        let tiny = plan("lowram", 2048, 8, 3072);
        assert!(tiny.xmx_mb >= 1024 && tiny.xms_mb <= tiny.xmx_mb);
        let p = plan("fps", 1024, 21, 32768);
        assert_eq!(p.xms_mb, 512);
    }

    #[test]
    fn flags_are_safe_across_java_versions() {
        for java in [8, 17, 21, 25] {
            for prof in ["fps", "lowram", "auto"] {
                let p = plan(prof, 4096, java, 16384);
                assert_eq!(p.flags[0], "-XX:+IgnoreUnrecognizedVMOptions", "must come first so later flags are covered");
                assert!(p.flags.iter().all(|x| x.starts_with("-XX:")));
                assert_eq!(p.flags.iter().any(|x| x.contains("CompactObjectHeaders")), java >= 24);
                // no flag listed twice
                let mut s = p.flags.clone(); s.sort(); s.dedup();
                assert_eq!(s.len(), p.flags.len());
            }
        }
    }

    #[test]
    fn only_the_fps_profile_raises_priority() {
        assert!(plan("fps", 4096, 21, 16384).high_priority);
        assert!(!plan("lowram", 4096, 21, 16384).high_priority);
        assert!(plan("auto", 4096, 21, 16384).high_priority); // 16 GB -> fps
        assert!(!plan("auto", 4096, 21, 8192).high_priority); // 8 GB -> lowram
    }

    #[test]
    fn recommendations_scale_with_mods_but_fit_the_pc() {
        assert_eq!(recommend_ram_mb(32768, "vanilla", 0), 3072);
        assert_eq!(recommend_ram_mb(32768, "fabric", 25), 4096);
        assert_eq!(recommend_ram_mb(32768, "fabric", 150), 8192);
        assert_eq!(recommend_ram_mb(8192, "fabric", 250), 4096); // half of an 8 GB PC
        assert_eq!(recommend_ram_mb(4096, "fabric", 250), 2048);
        assert!(recommend_ram_mb(1024, "vanilla", 0) >= 1024);
        for m in [0, 5, 30, 80, 150, 300] {
            assert_eq!(recommend_ram_mb(16384, "fabric", m) % 512, 0);
        }
    }

    #[test]
    fn reads_this_machines_memory() {
        assert!(total_ram_mb() >= 512);
    }
}
