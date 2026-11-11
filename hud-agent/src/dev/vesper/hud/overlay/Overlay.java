package dev.vesper.hud.overlay;

import org.lwjgl.glfw.GLFW;
import org.lwjgl.opengl.GL;

import java.io.File;
import java.io.InputStream;
import java.util.function.LongConsumer;

/**
 * The frame hook. The agent patches GLFW's swapBuffers to call accept(window) just before each frame is shown,
 * which is the one point every Minecraft version, on every mod loader, passes through. Everything here is wrapped
 * so a failure disables the overlay instead of ever reaching the game.
 */
public final class Overlay implements LongConsumer {
    private boolean dead, started, hidden;
    private int errors;
    private long window;
    private Font font;
    private GlSurface gl;
    private Config cfg = Config.defaults();
    private final Stats stats = new Stats();
    private final Input input = new Input();
    private long lastCfgCheck, cfgModified = -1;
    private final File cfgFile = new File(System.getProperty("vesper.hud.config", "config/vesper-hud.json"));
    private final File optionsFile = new File(System.getProperty("vesper.hud.options", "options.txt"));
    private final int[] fbw = new int[1], fbh = new int[1];

    @Override
    public void accept(long win) {
        if (dead) return;
        try {
            if (window == 0) window = win;
            if (win != window) return;
            long now = System.nanoTime();
            stats.frame(now);
            if (!started) start();
            reloadConfig(now);
            input.poll(window, now, optionsFile);
            if (input.f1Pressed) hidden = !hidden; // F1 hides the game's own HUD, so hide ours with it
            if (!cfg.enabled || hidden) return;
            GLFW.glfwGetFramebufferSize(window, fbw, fbh);
            if (fbw[0] < 64 || fbh[0] < 64) return;
            gl.begin(fbw[0], fbh[0]);
            try { new Hud(gl, font, stats, input, now).draw(cfg, fbw[0], fbh[0]); } finally { gl.end(); }
            errors = 0;
        } catch (Throwable t) {
            if (++errors >= 3) {
                dead = true;
                System.err.println("[Vesper HUD] overlay turned off: " + t);
            }
        }
    }

    private void start() throws Exception {
        started = true;
        GL.getCapabilities(); // throws when the game has no OpenGL context (for example a Vulkan renderer)
        if (!GL.getCapabilities().OpenGL32) throw new IllegalStateException("needs OpenGL 3.2");
        InputStream in = Overlay.class.getResourceAsStream("font.bin");
        if (in == null) throw new IllegalStateException("font missing");
        try { font = new Font(in); } finally { in.close(); }
        gl = new GlSurface(font);
        System.out.println("[Vesper HUD] overlay ready");
    }

    private void reloadConfig(long now) {
        if (now - lastCfgCheck < 1_000_000_000L && lastCfgCheck != 0) return;
        lastCfgCheck = now;
        try {
            if (!cfgFile.isFile()) return;
            long m = cfgFile.lastModified();
            if (m == cfgModified) return;
            cfg = Config.load(cfgFile);
            cfgModified = m;
        } catch (Exception e) {
            cfgModified = -1; // half-written or invalid: keep the settings we have, try again next second
        }
    }
}
