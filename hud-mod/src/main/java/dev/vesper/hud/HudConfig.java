package dev.vesper.hud;

import com.google.gson.Gson;
import net.fabricmc.loader.api.FabricLoader;

import java.io.Reader;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Settings written by the launcher's "In-game HUD" page. The field names are the JSON names, and the
 * defaults match the launcher's (src/js/hud-schema.js). The file is re-read when it changes, so edits in
 * the launcher show up in a running game.
 */
public final class HudConfig {
    public boolean enabled = true;
    public float scale = 1f;
    public Map<String, Element> elements = new LinkedHashMap<>();

    public static final class Element {
        public boolean enabled = false;
        public float x = 0.01f, y = 0.02f, scale = 1f;
        public String template = "";
        public String labelColor = "#c9bfff", valueColor = "#ffffff";
        public boolean rainbow = false, shadow = true;
        public String bg = "#0b0b17", border = "#8f7cff";
        public float bgOpacity = 0.55f, borderOpacity = 0.35f;
        public float borderWidth = 1f, radius = 4f, padding = 4f;

        Element(boolean on, float x, float y, String template) { this.enabled = on; this.x = x; this.y = y; this.template = template; }
        public Element() {}
    }

    private static final Path FILE = FabricLoader.getInstance().getConfigDir().resolve("vesper-hud.json");
    private static HudConfig current = defaults();
    private static long lastCheck, lastModified = -1;

    private static HudConfig defaults() {
        HudConfig c = new HudConfig();
        c.elements.put("fps", new Element(true, 0.01f, 0.02f, "FPS: {v}"));
        c.elements.put("cps", new Element(true, 0.01f, 0.10f, "CPS: {v}"));
        c.elements.put("ping", new Element(true, 0.01f, 0.18f, "Ping: {v}"));
        c.elements.put("coords", new Element(true, 0.01f, 0.26f, "XYZ: {v}"));
        c.elements.put("direction", new Element(false, 0.01f, 0.34f, "Facing: {v}"));
        c.elements.put("memory", new Element(false, 0.01f, 0.42f, "RAM: {v}"));
        c.elements.put("clock", new Element(false, 0.5f, 0.01f, "{v}"));
        c.elements.put("keystrokes", new Element(true, 0.01f, 0.72f, ""));
        c.elements.put("armor", new Element(true, 0.96f, 0.74f, ""));
        c.elements.put("potions", new Element(false, 0.99f, 0.02f, ""));
        return c;
    }

    /** The current settings; checks the file for changes about once a second. */
    public static HudConfig get() {
        long now = System.currentTimeMillis();
        if (now - lastCheck > 1000) {
            lastCheck = now;
            try {
                if (Files.exists(FILE)) {
                    long m = Files.getLastModifiedTime(FILE).toMillis();
                    if (m != lastModified) {
                        try (Reader r = Files.newBufferedReader(FILE)) {
                            HudConfig c = new Gson().fromJson(r, HudConfig.class);
                            if (c != null && c.elements != null) { current = c; }
                        }
                        lastModified = m;
                    }
                }
            } catch (Exception ignored) {
                // a half-written or invalid file: keep what we have and try again on the next change
                lastModified = -1;
            }
        }
        return current;
    }

    public static int rgb(String hex, int fallback) {
        try { return Integer.parseInt(hex.substring(1), 16) & 0xFFFFFF; } catch (Exception e) { return fallback; }
    }

    public static int argb(String hex, float opacity, int fallback) {
        int a = Math.round(Math.max(0f, Math.min(1f, opacity)) * 255f);
        return (a << 24) | rgb(hex, fallback);
    }
}
