# Vesper

A small, fast Minecraft: Java Edition launcher for Windows. Tauri (Rust + web UI), Modrinth built in.

## Legal

- Minecraft, its assets and libraries belong to Mojang/Microsoft. Vesper never bundles or redistributes them: everything is downloaded from Mojang's servers onto the user's PC, and the user must own Minecraft: Java Edition. There is no offline or cracked mode.
- Follow Mojang's [Commercial Usage Guidelines](https://www.minecraft.net/en-us/usage-guidelines) and do not use Mojang logos or imply endorsement.
- Modrinth content keeps its authors' licenses. Respect the Modrinth API terms and the User-Agent rule.
- Forge and NeoForge installers are downloaded from their official Maven servers and run locally.

## Rust mode

Besides Java, Vesper can run **Minecraft rewritten in Rust**: pick **Rust** as the loader when you create an instance. It is built from the open-source [Horizon](https://github.com/horizon-rust/horizon) project (MIT license, Copyright (c) 2026 Ludovic Marie and the Horizon contributors), renamed to Vesper by the [vesper-rust](https://github.com/Vesper-CO/vesper-rust) build repository.

- Vesper bundles none of it. On first Play it downloads the signed build from GitHub and checks its checksum before installing.
- Nothing is sent to Horizon's servers: the "playing now" ping, the cosmetics service and Discord presence are switched off.
- The build contains no Minecraft code or assets. Vesper downloads the Minecraft files from Mojang for your signed-in account (you must own Java Edition), and the game prepares itself from them. The first launch takes a few minutes.
- It needs a graphics card with Vulkan or DirectX 12, and no Java is used to play.
- Mods are turned off for Rust instances for now.

## Community

Join the Discord for help, feedback and updates: https://discord.gg/FpHGhHCwgU
