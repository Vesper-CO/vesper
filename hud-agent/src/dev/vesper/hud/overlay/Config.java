package dev.vesper.hud.overlay;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * The launcher's HUD settings (config/vesper-hud.json, written by the "In-game HUD" page). Anything missing or
 * out of range falls back to the launcher's defaults, so a hand-edited or old file never breaks the game.
 */
final class Config {
    static final class El {
        boolean enabled; float x, y, scale = 1f;
        String template = "";
        int labelColor = 0xC9BFFF, valueColor = 0xFFFFFF, bg = 0x0B0B17, border = 0x8F7CFF;
        boolean rainbow, shadow = true;
        float bgOpacity = 0.55f, borderOpacity = 0.35f, borderWidth = 1f, radius = 4f, padding = 4f;

        El(boolean on, float x, float y, String tpl) { enabled = on; this.x = x; this.y = y; template = tpl; }
    }

    boolean enabled = true;
    float scale = 1f;
    final Map<String, El> els = new LinkedHashMap<String, El>();

    /** The elements the agent can draw by itself: they need nothing from inside the game. */
    static final String[] IDS = { "fps", "cps", "frametime", "memory", "clock", "session", "keystrokes" };

    static Config defaults() {
        Config c = new Config();
        c.els.put("fps", new El(true, 0.01f, 0.02f, "FPS: {v}"));
        c.els.put("cps", new El(true, 0.01f, 0.10f, "CPS: {v}"));
        c.els.put("frametime", new El(false, 0.01f, 0.50f, "Frame: {v}"));
        c.els.put("memory", new El(false, 0.01f, 0.42f, "RAM: {v}"));
        c.els.put("clock", new El(false, 0.5f, 0.01f, "{v}"));
        c.els.put("session", new El(false, 0.01f, 0.58f, "Playtime: {v}"));
        c.els.put("keystrokes", new El(true, 0.01f, 0.72f, ""));
        return c;
    }

    static Config load(File f) throws Exception {
        String text = new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8);
        return parse(text);
    }

    @SuppressWarnings("unchecked")
    static Config parse(String text) {
        Config c = defaults();
        Object root = Json.parse(text);
        if (!(root instanceof Map)) return c;
        Map<String, Object> m = (Map<String, Object>) root;
        c.enabled = !Boolean.FALSE.equals(m.get("enabled"));
        c.scale = num(m.get("scale"), 0.5f, 3f, 1f);
        Object els = m.get("elements");
        if (!(els instanceof Map)) return c;
        for (Map.Entry<String, El> en : c.els.entrySet()) {
            Object o = ((Map<String, Object>) els).get(en.getKey());
            if (!(o instanceof Map)) continue;
            Map<String, Object> s = (Map<String, Object>) o;
            El e = en.getValue();
            if (s.get("enabled") instanceof Boolean) e.enabled = (Boolean) s.get("enabled");
            e.x = num(s.get("x"), 0f, 1f, e.x); e.y = num(s.get("y"), 0f, 1f, e.y);
            if (s.get("template") instanceof String && ((String) s.get("template")).length() <= 60) e.template = (String) s.get("template");
            e.scale = num(s.get("scale"), 0.5f, 3f, e.scale);
            e.labelColor = hex(s.get("labelColor"), e.labelColor); e.valueColor = hex(s.get("valueColor"), e.valueColor);
            e.bg = hex(s.get("bg"), e.bg); e.border = hex(s.get("border"), e.border);
            e.bgOpacity = num(s.get("bgOpacity"), 0f, 1f, e.bgOpacity); e.borderOpacity = num(s.get("borderOpacity"), 0f, 1f, e.borderOpacity);
            e.borderWidth = num(s.get("borderWidth"), 0f, 4f, e.borderWidth);
            e.radius = num(s.get("radius"), 0f, 12f, e.radius); e.padding = num(s.get("padding"), 0f, 12f, e.padding);
            e.rainbow = Boolean.TRUE.equals(s.get("rainbow")); e.shadow = !Boolean.FALSE.equals(s.get("shadow"));
        }
        return c;
    }

    private static float num(Object o, float lo, float hi, float d) {
        if (!(o instanceof Number)) return d;
        float v = ((Number) o).floatValue();
        return Float.isNaN(v) ? d : Math.max(lo, Math.min(hi, v));
    }

    private static int hex(Object o, int d) {
        if (!(o instanceof String)) return d;
        String s = (String) o;
        if (s.length() != 7 || s.charAt(0) != '#') return d;
        try { return Integer.parseInt(s.substring(1), 16); } catch (NumberFormatException e) { return d; }
    }
}
