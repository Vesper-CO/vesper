package dev.vesper.hud.overlay;

import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;

import java.io.File;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.function.LongConsumer;

/**
 * The frame hook. The agent patches GLFW's swapBuffers to call accept(window) just before each frame is shown,
 * which is the one point every Minecraft version, on every mod loader, passes through. Everything here is wrapped
 * so a failure disables the overlay instead of ever reaching the game.
 *
 * Edit mode: with a menu open (the pointer is free), Right Shift lets you drag the elements, + and - resize the
 * one under the pointer, and Right Shift again (or going back to the game) saves.
 */
public final class Overlay implements LongConsumer {
    private final Platform plat;
    private final String platformName;
    private boolean dead, started, hidden, firstDrawLogged, heartbeatLogged;
    private int errors, frames;
    private long window;
    private Font font;
    private GlSurface gl;
    private Config cfg = Config.defaults();
    private final Stats stats = new Stats();
    private final Input input = new Input();
    private long lastCfgCheck, cfgModified = -1;
    private final File cfgFile = new File(System.getProperty("vesper.hud.config", "config/vesper-hud.json"));
    private final File optionsFile = new File(System.getProperty("vesper.hud.options", "options.txt"));
    private final int[] fbw = new int[1], fbh = new int[1], winw = new int[1], winh = new int[1];
    private final double[] curx = new double[1], cury = new double[1];

    // edit mode
    private boolean edit, dirty, wasDown;
    private String dragId, hoverId, hint;
    private float dragDx, dragDy;
    private long hintUntil;
    private final Set<String> edited = new HashSet<String>();
    private List<Hud.Hit> hits = new ArrayList<Hud.Hit>();

    /** @param platform "glfw" (Minecraft up to 1.21.x) or "sdl" (26.x) */
    public Overlay(String platform) {
        platformName = platform;
        plat = "sdl".equals(platform) ? new SdlPlatform() : new GlfwPlatform();
    }

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
            input.poll(plat, window, now, optionsFile);
            if (input.f1Pressed) hidden = !hidden; // F1 hides the game's own HUD, so hide ours with it
            boolean free = plat.cursorFree(window);
            if (input.rshiftPressed) {
                if (edit) endEdit();
                else if (free) startEdit();
                else { hint = "Open the pause menu (Esc), then press Right Shift to edit the HUD"; hintUntil = now + 3_000_000_000L; }
            }
            if (edit && !free) endEdit(); // back in the game: finish and save
            boolean hintOn = hint != null && now < hintUntil;
            boolean showHud = (cfg.enabled && !hidden) || edit;
            if (!showHud && !hintOn) return;
            plat.framebufferSize(window, fbw, fbh);
            int W = fbw[0], H = fbh[0];
            if (W < 64 || H < 64) return;
            gl.begin(W, H);
            try {
                Hud hud = new Hud(gl, font, stats, input, now);
                if (showHud) hud.draw(cfg, W, H); else hud.unit = Math.max(0.5f, Math.min(W / 480f, H / 270f));
                hits = hud.hits;
                if (edit) {
                    editFrame(W, H);
                    hud.editChrome(W, H, dragId != null ? dragId : hoverId,
                        cfg.enabled ? "HUD editor: drag an element, + / - to resize, Right Shift to save" : "The HUD is turned off in the launcher, so nothing is shown");
                } else if (hintOn) hud.banner(W, hint);
            } finally { gl.end(); }
            if (!firstDrawLogged) { firstDrawLogged = true; Log.info("first frame drawn: " + gl.lastQuads() + " quads at " + W + "x" + H); }
            if (!heartbeatLogged && ++frames == 300) { heartbeatLogged = true; Log.info("running: about " + stats.fps + " fps, " + hits.size() + " elements on screen"); }
            errors = 0;
        } catch (Throwable t) {
            if (errors == 0) Log.error("problem while drawing (the overlay stops after 3 in a row)", t);
            if (++errors >= 3) { dead = true; Log.info("overlay turned off after repeated errors; see above"); }
        }
    }

    // ----- edit mode -----

    private void startEdit() { edit = true; dirty = false; dragId = null; edited.clear(); Log.info("edit mode on"); }

    private void endEdit() {
        edit = false; dragId = null; hoverId = null;
        if (dirty) save();
        Log.info("edit mode off");
    }

    private void save() {
        try {
            cfg.saveEdits(cfgFile, edited);
            cfgModified = cfgFile.lastModified();
            dirty = false;
        } catch (Exception e) { Log.error("could not save the HUD settings", e); }
    }

    private Hud.Hit hit(String id) { for (Hud.Hit h : hits) if (h.id.equals(id)) return h; return null; }

    private void editFrame(int W, int H) {
        plat.windowSize(window, winw, winh);
        plat.cursor(window, curx, cury);
        float mx = (float) curx[0] * (winw[0] > 0 ? (float) W / winw[0] : 1f), my = (float) cury[0] * (winh[0] > 0 ? (float) H / winh[0] : 1f);
        boolean down = plat.mouse(window, 0);
        hoverId = null;
        for (int i = hits.size() - 1; i >= 0; i--) if (hits.get(i).contains(mx, my)) { hoverId = hits.get(i).id; break; }
        if (down && !wasDown && hoverId != null) { Hud.Hit h = hit(hoverId); dragId = hoverId; dragDx = mx - h.x; dragDy = my - h.y; }
        if (down && dragId != null) {
            Hud.Hit h = hit(dragId);
            Config.El e = cfg.els.get(dragId);
            if (h != null && e != null) {
                e.x = snap(clamp01((mx - dragDx) / Math.max(1f, W - h.w))); e.y = snap(clamp01((my - dragDy) / Math.max(1f, H - h.h)));
                edited.add(dragId); dirty = true;
            }
        }
        if (!down && wasDown) { dragId = null; if (dirty) save(); }
        wasDown = down;
        String target = dragId != null ? dragId : hoverId;
        if (target != null && (input.plusPressed || input.minusPressed)) {
            Config.El e = cfg.els.get(target);
            if (e != null) { e.scale = Math.max(0.5f, Math.min(3f, e.scale * (input.plusPressed ? 1.1f : 1f / 1.1f))); edited.add(target); dirty = true; save(); }
        }
    }

    private static float clamp01(float v) { return Math.max(0f, Math.min(1f, v)); }

    /** Stick to the screen's edges and centre line when close. */
    private static float snap(float v) {
        if (Math.abs(v - 0.5f) < 0.015f) return 0.5f;
        if (v < 0.01f) return 0f;
        if (v > 0.99f) return 1f;
        return Math.round(v * 10000f) / 10000f;
    }

    // ----- setup -----

    private void start() throws Exception {
        started = true;
        File dir = cfgFile.getAbsoluteFile().getParentFile();
        Log.init(new File(dir != null && dir.getParentFile() != null ? dir.getParentFile() : new File("."), "vesper-hud.log"));
        Log.info("overlay starting on " + platformName + " (config " + cfgFile.getAbsolutePath() + ")");
        GL.getCapabilities(); // throws when the game has no OpenGL context (for example a Vulkan renderer)
        Log.info("OpenGL: " + GL11.glGetString(0x1F02) + " on " + GL11.glGetString(0x1F01));
        if (!GL.getCapabilities().OpenGL32) throw new IllegalStateException("needs OpenGL 3.2");
        InputStream in = Overlay.class.getResourceAsStream("font.bin");
        if (in == null) throw new IllegalStateException("font missing");
        try { font = new Font(in); } finally { in.close(); }
        gl = new GlSurface(font);
        Log.info("overlay ready");
    }

    private void reloadConfig(long now) {
        if (now - lastCfgCheck < 1_000_000_000L && lastCfgCheck != 0) return;
        lastCfgCheck = now;
        if (edit) return; // never replace what is being dragged
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
