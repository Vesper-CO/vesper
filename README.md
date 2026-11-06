# Vesper

A small, fast Minecraft: Java Edition launcher for Windows. Tauri (Rust + web UI), Modrinth built in.

> Vesper is unofficial. It is not approved by or associated with Mojang or Microsoft.

## Why Tauri instead of Electron

| | Tauri | Electron |
|---|---|---|
| Installer | roughly 3-8 MB | roughly 80-110 MB |
| Idle RAM | uses the WebView2 already in Windows 10/11 | ships and runs its own Chromium |
| Startup | fast | slower |
| Heavy work (parallel downloads, hashing, processes) | Rust, native threads | Node |

A launcher is mostly downloading, hashing and spawning Java, which Rust does well. The UI is plain HTML/CSS/JS with no framework and no bundler.

## Features

- Microsoft sign-in (official OAuth device-code flow), several accounts. Tokens are AES-256-GCM encrypted on disk; the key lives in Windows Credential Manager. No passwords are ever handled.
- Versions: releases, snapshots, old versions. Loaders: Fabric, Quilt, Forge, NeoForge.
- Game, libraries, assets and the right Java runtime come from Mojang's official manifests. Downloads are parallel (16 at once), resumable (HTTP Range) and SHA-1 verified.
- Instances with their own version, loader, mods, RAM and JVM arguments.
- Modrinth: search with filters, project pages (description, gallery, versions, changelog), one-click install with required dependencies, compatibility checks, update-all, enable/disable. Mods, resource packs, shaders, modpacks and `.mrpack` import.
- Live game console, skeleton loaders, dark theme (default) and light theme.
- Per-user installer (no admin), auto-update.

## Setup

Prerequisites: Windows 10/11, [Node.js 20+](https://nodejs.org), [Rust](https://rustup.rs) with the MSVC toolchain, and "Desktop development with C++" from Visual Studio Build Tools.

### 1. Install and run

```powershell
npm install
npm run dev
```

### 2. Register a Microsoft app (needed for sign-in)

1. In the Azure portal create an App registration. Supported account types: "Personal Microsoft accounts only".
2. Authentication tab: turn on **Allow public client flows**.
3. Copy the Application (client) ID into Vesper > Settings > Microsoft sign-in (or set `VESPER_MS_CLIENT_ID` when building).
4. Minecraft's API only accepts apps that Mojang has approved. Request approval through the form linked from the "Microsoft authentication scheme" page on the Minecraft Wiki. Until then, sign-in fails at the Minecraft step.

### 3. Fill in the placeholders

- `src-tauri/tauri.conf.json`: `plugins.updater.endpoints` (your GitHub name) and `pubkey` (next step).
- `src-tauri/src/download.rs`: the User-Agent contact URL. Modrinth asks for a real one.

### 4. Create the update-signing key (once)

```powershell
npx tauri signer generate -w vesper.key
```

Paste the **public** key into `pubkey`. Keep `vesper.key` private; never commit it.

## Build the installer

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content vesper.key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "your-password"
npm run build
```

Output: `src-tauri\target\release\bundle\nsis\Vesper_0.1.0_x64-setup.exe`

It is an NSIS installer with `installMode: currentUser` (no admin prompt). It adds a Start Menu entry and an uninstaller, offers to launch Vesper on the final page, and installs the WebView2 runtime if missing. Check the finish page in your Tauri version for the desktop-shortcut option.

## GitHub Actions and auto-update

`.github/workflows/release.yml` builds on every `v*` tag and publishes the installer plus `latest.json` to a GitHub release.

1. Add repository secrets `TAURI_SIGNING_PRIVATE_KEY` (contents of `vesper.key`) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
2. Bump `version` in `tauri.conf.json` and `Cargo.toml`, then `git tag v0.1.1 && git push --tags`.
3. Installed copies check `releases/latest/download/latest.json` at startup, then download, verify the signature, install and restart (Settings can turn this off).

## Project layout

```
src/                     frontend (no build step)
  index.html, styles.css
  js/  main.js shell.js state.js api.js util.js
  js/views/  home instances mods accounts settings
src-tauri/
  tauri.conf.json  capabilities/default.json
  src/
    main.rs        command registration, updater, link opening
    download.rs    parallel + resumable + verified downloads
    mojang.rs      manifest, libraries, assets, Java runtime, news
    loaders.rs     Fabric / Quilt / Forge / NeoForge
    launch.rs      JVM command line, process, log streaming
    auth.rs        Microsoft -> Xbox -> Minecraft, encrypted tokens
    instances.rs   profiles          modrinth.rs  library API
    mrpack.rs      .mrpack import    settings.rs  paths.rs  error.rs
```

Data lives in `%APPDATA%\Vesper`. Game files are shared between instances; each instance has its own `game` folder.

## Legal

- Minecraft, its assets and libraries belong to Mojang/Microsoft. Vesper never bundles or redistributes them: everything is downloaded from Mojang's servers onto the user's PC, and the user must own Minecraft: Java Edition. There is no offline or cracked mode.
- Follow Mojang's [Commercial Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines) and do not use Mojang logos or imply endorsement.
- Modrinth content keeps its authors' licenses. Respect the Modrinth API terms and the User-Agent rule.
- Forge and NeoForge installers are downloaded from their official Maven servers and run locally.
- Vesper's own code: choose your license (none is included).

## Status and known limits

This code was written without access to a Rust compiler or the live APIs, so it has **not been compiled or run**. Expect to fix a few compile errors on the first `cargo build`, and check the Forge/NeoForge/Modrinth endpoint shapes. Not yet implemented: Windows ARM Java, per-instance skins, Mojang log4j config files, mod search paging beyond "Load more", crash-report analysis.
