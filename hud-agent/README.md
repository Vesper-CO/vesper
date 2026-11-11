# Vesper HUD agent

The universal in-game HUD. It is a Java agent (`-javaagent`) that the launcher embeds and starts for any
instance with the HUD switched on, on **any mod loader and any Minecraft version from 1.13 up** (including
year-style versions such as 26.x).

## How it works

* `Agent` (loaded by the JVM) adds one call to the start of LWJGL's `GLFW.glfwSwapBuffers(long)` with ASM.
  Every Minecraft frame since 1.13 goes through that method, whatever the loader, and it is LWJGL's own class,
  not a (renamed) Minecraft one. The call only names `java.base` types, so it works under Fabric's Knot,
  Forge/NeoForge's module loaders, or none.
* The first frame loads `overlay.*` in a child of the loader that holds GLFW, so the overlay can see LWJGL.
* Each frame the overlay reads the settings (`config/vesper-hud.json`, written by the launcher), reads keys and
  mouse buttons from GLFW, lays the elements out and draws them with a small OpenGL 3.2 program, then puts back
  all GL state it touched. Any error turns the overlay off; it never reaches the game.

## What it can show

Only what can be measured from outside the game: FPS, CPS, frame time, memory, clock, playtime and keystrokes
(following the player's own key bindings in `options.txt`). Coordinates, ping, direction, armor and potion effects
live inside the game, in classes whose names differ per version and loader, so they come from the optional
Fabric add-on (`/hud-mod`), which skips the elements the agent already draws.

## Building

`./build.sh` (JDK 17+ and ASM 9.x; on Ubuntu `apt install libasm-java`) compiles against the tiny LWJGL stubs in
`stubs/` (never shipped), moves ASM to `dev.vesper.hud.shaded.asm` so it cannot clash with the game's own ASM, and
copies the jar to `src-tauri/resources/`, which the launcher embeds. `tools/BakeFont.java` regenerates the font atlas.

## Tests

`test/AgentMain.java` runs a fake GLFW in its own class loader under the real agent and checks that the patched
call reaches the overlay and draws. `test/.../RenderTest.java` draws the overlay to a PNG with a software
rasteriser. The OpenGL calls themselves can only be checked in a real game.
