# Vesper HUD agent

The universal in-game HUD. It is a Java agent (`-javaagent`) that the launcher embeds and starts for any
instance with the HUD switched on, on **any mod loader and any Minecraft version from 1.13 up** (including
year-style versions such as 26.x).

## How it works

* `Agent` (loaded by the JVM) adds one call to the start of the function that presents each frame, with ASM:
  LWJGL's `GLFW.glfwSwapBuffers(long)` for Minecraft 1.13 to 1.21.x, and `SDLVideo.SDL_GL_SwapWindow(long)` for
  Minecraft 26.x, which opens its window with SDL3 instead of GLFW (its log says "SDL video driver"). Both are
  LWJGL's own classes, not (renamed) Minecraft ones, so the loader does not matter. The call only names `java.base` types, so it works under Fabric's Knot,
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

## Editing in game

With a menu open (Esc), press **Right Shift**: every element gets an outline and a panel opens on the right with the
same settings as the launcher's In-game HUD page, in four small tabs:

* **Elements**: show or hide each one, pick which to edit, and apply a style preset to all (Vesper Night, Vesper Day,
  Glass, Transparent, Neon).
* **Text**: label and value colour (16 swatches plus a hue strip), rainbow text, shadow, size.
* **Background**: colour, opacity, and "Fully transparent" / "Solid".
* **Border**: colour, opacity, width, corner roundness, padding.

Drag any element to move it (it snaps to the edges and centre); hover one and press **+** or **-** to resize it.
"Use for all" copies the look of the selected element to the others, "Reset" restores its defaults, and "Done" (or
Right Shift again, or going back to the game) finishes. Changes save when you let go of the mouse into
`config/vesper-hud.json`, and the launcher uses that file as its own copy. The text of an element ({v} templates) is
edited in the launcher, because typing needs the keyboard. Clicks still reach the menu underneath, so avoid starting a
drag on a button.

## When nothing shows

The agent prints `[Vesper HUD]` lines to the game's console and to `vesper-hud.log` in the game folder. In order, a
healthy start prints: `agent loaded`, `hooked org.lwjgl.sdl.SDLVideo.SDL_GL_SwapWindow` (or `...glfw.GLFW.glfwSwapBuffers`), `first frame reached the hook`, `OpenGL: ...`,
`overlay ready`, `first frame drawn`. Where it stops says why: no first line means the agent was not started; no
second means GLFW was never loaded through the normal path; an error after `overlay starting` names the OpenGL call or
the missing OpenGL context (a renderer that is not OpenGL cannot be drawn on this way).
