#!/usr/bin/env python3
"""Renames Horizon's client to Vesper and switches off everything that talks to Horizon's servers.

Usage: python rebrand.py <unpacked horizon source folder>

Every edit is checked: a REQUIRED edit that no longer matches (Horizon changed that code) stops the build,
so a half-rebranded client is never published. OPTIONAL edits only warn.
"""
import pathlib, re, sys

root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "src")
CLIENT_CRATES = ["horizon-client", "horizon-client-render", "horizon-client-core", "horizon-client-input"]
failures, done = [], []


def edit(rel, pattern, repl, required=True, regex=False, flags=0):
    p = root / rel
    if not p.exists():
        (failures if required else done).append(f"missing file {rel}")
        return
    text = p.read_text(encoding="utf-8")
    new, n = (re.subn(pattern, repl, text, flags=flags) if regex else (text.replace(pattern, repl), text.count(pattern)))
    if n == 0:
        (failures if required else done).append(f"{'REQUIRED' if required else 'optional'} edit not found in {rel}: {pattern[:60]!r}")
        return
    p.write_text(new, encoding="utf-8")
    done.append(f"ok  {rel} ({n})")


# ---- window titles, boot splash, title screen -------------------------------------------------
for rel in ["crates/horizon-client/src/game.rs", "crates/horizon-client/src/game/no_resources.rs"]:
    edit(rel, '.with_title("Horizon")', '.with_title("Vesper")')
edit("crates/horizon-client/src/game/split/windows.rs", '"Horizon', '"Vesper')
edit("crates/horizon-client/src/ui_theme.rs", 'BOOT_TEXT, "Horizon"', 'BOOT_TEXT, "Vesper"')

# The version line on the title screen ("Horizon 26.x (Rust)") and the nightly label in the corner.
edit("crates/horizon-client/src/title.rs",
     r'let version = format!\("Horizon \{\} \(Rust\)",[^;]*;\s*batch\.text\(atlas, &version[^;]*;', "", regex=True)
edit("crates/horizon-client/src/title.rs",
     r'if let Some\(nightly\) = horizon_core::release::nightly_label\(\) \{\s*batch\.text\(atlas, &nightly[^;]*;\s*\}', "", regex=True)

# ---- settings -----------------------------------------------------------------------------------
edit("crates/horizon-client-render/src/settings_shell.rs", 'Text::literal("Horizon")', 'Text::literal("Vesper")')

# ---- nothing is sent to Horizon's servers ---------------------------------------------------------
edit("crates/horizon-telemetry/src/lib.rs", "        _ => Some(DEFAULT_ENDPOINT.to_owned()),", "        _ => None,")
edit("crates/horizon-cosmetics/src/lib.rs", "pub const ON_BY_DEFAULT: bool = true;", "pub const ON_BY_DEFAULT: bool = false;")
edit("crates/horizon-client/src/discord_presence.rs", "pub fn start(enabled: bool, automated: bool) {\n",
     "pub fn start(enabled: bool, automated: bool) {\n    if true {\n        return; // Vesper has no Discord application of its own\n    }\n")

# ---- remaining visible text and links -------------------------------------------------------------
edit("crates/horizon-client/src/no_resources.rs", "Download the launcher at playhorizon.dev, then press Play there.", "Open Vesper and press Play there.", required=False)
edit("crates/horizon-client/src/lan/mod.rs", "This server is for the Horizon client: playhorizon.dev", "This server is for the Vesper client: github.com/jakedev2/vesper", required=False)
edit("crates/horizon-client/Cargo.toml", 'description = "Horizon native development client"', 'description = "Vesper"', required=False)

# Any other string that starts with the name: menus, tooltips, logs, the F3 screen, user agents.
count = 0
for crate in CLIENT_CRATES:
    for f in (root / "crates" / crate / "src").rglob("*.rs"):
        t = f.read_text(encoding="utf-8")
        n = re.sub(r'"Horizon(?=["\s:{/])', '"Vesper', t)
        if n != t:
            f.write_text(n, encoding="utf-8"); count += 1
done.append(f"ok  text strings in {count} client source files")

# Language files, every language.
langs = list((root / "crates/horizon-client/assets/horizon/lang").glob("*.json"))
for f in langs:
    t = f.read_text(encoding="utf-8")
    f.write_text(re.sub(r"\bHorizon\b", "Vesper", t), encoding="utf-8")
done.append(f"ok  {len(langs)} language files")

print("\n".join(done))
if failures:
    print("\nFAILED:\n" + "\n".join(failures))
    sys.exit(1)
