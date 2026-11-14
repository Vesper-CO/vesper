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

Open the pause menu (Esc) and click the **V** button next to the game's own small icon buttons (in 26.x it joins the
row of bug / feedback / friends / report icons; in older versions it sits beside "Back to Game"). Hovering shows
"Vesper HUD editor". Right Shift in any menu still works as a shortcut. Every element gets an outline and the Vesper
menu opens on the right, in four small tabs:

* **Elements**: a switch for the whole HUD, a switch per element, and the size of the selected one.
* **Looks**: Vesper Night, Vesper Day, Glass, Transparent and Neon, each shown as a live sample. One click restyles all.
* **Colors**: one picker for whichever part you choose (Label, Value, Fill or Border), rainbow text, shadow, opacity.
* **Shape**: border width, corner roundness, padding.

The **Editing** row picks the element the next change goes to (arrows), or **All** to change every element at once.
Drag any element to move it (it snaps to the edges and centre); hover one and press **+** or **-** to resize it.
**Reset** restores the defaults, and **Done** (or the x, or leaving the menu) finishes. Changes save when you let go
of the mouse into `config/vesper-hud.json`, and the launcher uses that file as its own copy. The text of an element
({v} templates) is edited in the launcher, because typing needs the keyboard.

The button is placed by calculation (`PauseMenu.java`): the pause menu is laid out the same way in every version, from
the window size and the GUI scale in options.txt. The button only shows while the pause menu is open (the overlay sees
Esc open it) and hides when you open another screen from it. On a server whose pause menu has an extra row (server
links) the button can sit one row off. Clicks still reach the menu underneath, so the editor panel stays in the empty
space to the right of the game's buttons.

## When nothing shows

The agent prints `[Vesper HUD]` lines to the game's console and to `vesper-hud.log` in the game folder. In order, a
healthy start prints: `agent loaded`, `hooked org.lwjgl.sdl.SDLVideo.SDL_GL_SwapWindow` (or `...glfw.GLFW.glfwSwapBuffers`), `first frame reached the hook`, `OpenGL: ...`,
`overlay ready`, `first frame drawn`. Where it stops says why: no first line means the agent was not started; no
second means GLFW was never loaded through the normal path; an error after `overlay starting` names the OpenGL call or
the missing OpenGL context (a renderer that is not OpenGL cannot be drawn on this way).
