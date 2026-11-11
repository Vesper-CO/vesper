# Vesper HUD (Fabric mod)

The overlay that the launcher's **In-game HUD** page styles: FPS, CPS, ping, coordinates, direction,
memory, clock, keystrokes, armor status and potion effects. Every element has its own colours,
background opacity (down to fully transparent), border, corner roundness, size and position.

The mod only draws. All settings live in `config/vesper-hud.json`, written by the launcher before each
launch and re-read about once a second while the game runs.

## Building

```
cd hud-mod
gradle build        # jar in build/libs
```

One build per Minecraft version (the GUI code changes between versions): edit `gradle.properties`
(`minecraft_version`, `yarn_mappings`, `fabric_version`) and add the version to `HUD_VERSIONS` in
`src-tauri/src/hud.rs`. The `HUD mod` GitHub workflow builds the jar and publishes it, with a SHA-256
file, to the `hud-latest` release, which is where the launcher downloads it from.

Currently written for **Minecraft 1.21.4**.
