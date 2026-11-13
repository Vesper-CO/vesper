package dev.vesper.hud.overlay;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * The in-game customiser, drawn over the game in edit mode. It offers the same settings as the launcher's
 * In-game HUD page, grouped into four small tabs so only a few controls are on screen at once:
 * Elements (what to show, presets), Text (colours, rainbow, shadow, size), Background and Border.
 * It is immediate-mode: every frame it draws itself and reports clicks, so it needs no state beyond the tab.
 */
final class Panel {
    static final String[] TABS = { "Elements", "Text", "Background", "Border" };
    static final int[] PALETTE = {
        0xFFFFFF, 0xC8CCD8, 0x7A7F90, 0x0B0B17, 0xFF4D5A, 0xFFA020, 0xFFE27A, 0xA4E84A,
        0x3DDC84, 0x3DFCFF, 0x5AA8FF, 0x3A6BFF, 0x8F7CFF, 0xFF5CE1, 0xFFA6C9, 0x8A6038 };
    static final String[] NAMES = { "FPS", "CPS", "Frame time", "Memory", "Clock", "Playtime", "Keystrokes" };
    static final String[] PRESET_NAMES = { "Vesper Night", "Vesper Day", "Glass", "Transparent", "Neon" };

    int tab;
    String selected = "fps";
    /** widget id -> x, y, w, h in pixels (hit areas; the tests click through these) */
    final Map<String, float[]> rects = new LinkedHashMap<String, float[]>();
    float px, py, pw, ph;

    private Hud hud;
    private float u, mx, my, fs;
    private boolean down, pressed;
    private String active; // the slider or colour strip being dragged
    private boolean changed;
    private Config cfg;
    private Set<String> edited;

    boolean contains(float x, float y) { return x >= px && x <= px + pw && y >= py && y <= py + ph; }

    /** Draw the panel and handle the pointer. Returns true when a setting changed. */
    boolean frame(Hud hud, Config cfg, Set<String> edited, int W, int H, float mx, float my, boolean down, boolean pressed) {
        this.hud = hud; this.cfg = cfg; this.edited = edited; this.mx = mx; this.my = my; this.down = down; this.pressed = pressed;
        if (!down) active = null;
        changed = false;
        rects.clear();
        u = hud.unit; fs = 6.5f * u;
        pw = 150f * u; ph = 158f * u; px = W - pw - 4f * u; py = 8f * u;
        Config.El el = cfg.els.get(selected);
        if (el == null) { selected = "fps"; el = cfg.els.get("fps"); }

        hud.fill(px, py, pw, ph, 4f * u, 0xF20B0B17);
        hud.fill(px, py, pw, u, 0f, 0xFF8F7CFF);
        float pad = 7f * u, x = px + pad, w = pw - 2 * pad, y = py + 5f * u;
        hud.text("HUD editor", x, y, 9f * u, 7.5f * u, 0xFFFFFFFF);
        String sub = niceName(selected);
        hud.text(sub, px + pw - pad - hud.textWidth(sub, fs), y, 9f * u, fs, 0xFFC9BFFF);
        y += 12f * u;

        // tabs
        float tw = (w - 3 * u) / 4f;
        for (int i = 0; i < 4; i++) if (button("tab:" + i, TABS[i], x + i * (tw + u), y, tw, 10f * u, i == tab)) tab = i;
        y += 15f * u;

        if (tab == 0) elementsTab(x, y, w, el);
        else if (tab == 1) textTab(x, y, w, el);
        else if (tab == 2) backgroundTab(x, y, w, el);
        else borderTab(x, y, w, el);

        // footer
        float fy = py + ph - 15f * u, bw = (w - 2 * u) / 3f;
        if (button("all", "Use for all", x, fy, bw, 10f * u, false)) { for (Map.Entry<String, Config.El> e : cfg.els.entrySet()) if (e.getValue() != el) { e.getValue().copyStyle(el, false); edited.add(e.getKey()); } touch(); }
        if (button("reset", "Reset", x + bw + u, fy, bw, 10f * u, false)) { Config.El d = Config.defaults().els.get(selected); if (d != null) { el.copyStyle(d, true); touch(); } }
        if (button("done", "Done", x + 2 * (bw + u), fy, bw, 10f * u, true)) done = true;
        return changed;
    }

    /** Set when the player presses Done. */
    boolean done;

    // ----- tabs -----

    private void elementsTab(float x, float y, float w, Config.El sel) {
        hud.text("Click a row or an element on screen", x, y - 1f * u, 8f * u, 5.5f * u, 0xFF9A98B8);
        y += 8f * u;
        for (int i = 0; i < Config.IDS.length; i++) {
            String id = Config.IDS[i];
            Config.El e = cfg.els.get(id);
            boolean on = id.equals(selected);
            float rh = 9.5f * u;
            rects.put("row:" + id, new float[] { x, y, w - 20f * u, rh });
            boolean over = hit(x, y, w - 20f * u, rh);
            hud.fill(x, y, w, rh, 2f * u, on ? 0x558F7CFF : over ? 0x33FFFFFF : 0x00000000);
            hud.text(niceName(id), x + 3f * u, y, rh, fs, 0xFFFFFFFF);
            if (pressed && over) selected = id;
            boolean now = switchWidget("show:" + id, x + w - 17f * u, y + 1.5f * u, e.enabled);
            if (now != e.enabled) { e.enabled = now; edited.add(id); touch(); }
            y += 10.5f * u;
        }
        y += 2f * u;
        hud.text("Style for all elements", x, y, 8f * u, 5.5f * u, 0xFF9A98B8);
        y += 8f * u;
        float bw = (w - 2 * u) / 3f;
        for (int i = 0; i < PRESET_NAMES.length; i++) {
            float bx = x + (i % 3) * (bw + u), by = y + (i / 3) * 11f * u;
            if (button("preset:" + i, PRESET_NAMES[i], bx, by, bw, 10f * u, false)) { applyPreset(i); touch(); }
        }
    }

    private void textTab(float x, float y, float w, Config.El e) {
        int c = colour("Label colour", "label", x, y, w, e.labelColor); if (c >= 0) { e.labelColor = c; mark(); }
        y += 38f * u;
        c = colour("Value colour", "value", x, y, w, e.valueColor); if (c >= 0) { e.valueColor = c; mark(); }
        y += 38f * u;
        boolean r = toggle("rainbow", "Rainbow text", x, y, w / 2f - u, e.rainbow); if (r != e.rainbow) { e.rainbow = r; mark(); }
        boolean s = toggle("shadow", "Text shadow", x + w / 2f + u, y, w / 2f - u, e.shadow); if (s != e.shadow) { e.shadow = s; mark(); }
        y += 13f * u;
        float v = slider("size", "Size", x, y, w, e.scale, 0.5f, 3f, 0.05f, false); if (v != e.scale) { e.scale = v; mark(); }
    }

    private void backgroundTab(float x, float y, float w, Config.El e) {
        int c = colour("Background colour", "bg", x, y, w, e.bg); if (c >= 0) { e.bg = c; mark(); }
        y += 38f * u;
        float v = slider("bgop", "Opacity", x, y, w, e.bgOpacity, 0f, 1f, 0.01f, true); if (v != e.bgOpacity) { e.bgOpacity = v; mark(); }
        y += 16f * u;
        float bw = (w - u) / 2f;
        if (button("clear", "Fully transparent", x, y, bw, 10f * u, e.bgOpacity == 0f)) { e.bgOpacity = 0f; mark(); }
        if (button("solid", "Solid", x + bw + u, y, bw, 10f * u, e.bgOpacity == 1f)) { e.bgOpacity = 1f; mark(); }
    }

    private void borderTab(float x, float y, float w, Config.El e) {
        int c = colour("Border colour", "border", x, y, w, e.border); if (c >= 0) { e.border = c; mark(); }
        y += 38f * u;
        float v = slider("bop", "Border opacity", x, y, w, e.borderOpacity, 0f, 1f, 0.01f, true); if (v != e.borderOpacity) { e.borderOpacity = v; mark(); }
        y += 15f * u;
        v = slider("bw", "Border width", x, y, w, e.borderWidth, 0f, 4f, 0.5f, false); if (v != e.borderWidth) { e.borderWidth = v; mark(); }
        y += 15f * u;
        v = slider("rad", "Corner roundness", x, y, w, e.radius, 0f, 12f, 1f, false); if (v != e.radius) { e.radius = v; mark(); }
        y += 15f * u;
        v = slider("pad", "Padding", x, y, w, e.padding, 0f, 12f, 1f, false); if (v != e.padding) { e.padding = v; mark(); }
    }

    private void applyPreset(int i) {
        for (Map.Entry<String, Config.El> en : cfg.els.entrySet()) {
            Config.El e = en.getValue();
            e.labelColor = 0xC9BFFF; e.valueColor = 0xFFFFFF; e.rainbow = false; e.shadow = true;
            e.bg = 0x0B0B17; e.bgOpacity = 0.55f; e.border = 0x8F7CFF; e.borderOpacity = 0.35f; e.borderWidth = 1f; e.radius = 4f; e.padding = 4f;
            switch (i) {
                case 0: e.radius = 5f; break;
                case 1: e.labelColor = 0x1F64D6; e.valueColor = 0x14213D; e.shadow = false; e.bg = 0xFFFFFF; e.bgOpacity = 0.62f; e.border = 0x1F64D6; e.radius = 5f; break;
                case 2: e.labelColor = 0xE8EEFF; e.bg = 0xFFFFFF; e.bgOpacity = 0.12f; e.border = 0xFFFFFF; e.borderOpacity = 0.4f; e.radius = 7f; break;
                case 3: e.labelColor = 0xFFFFFF; e.bgOpacity = 0f; e.borderOpacity = 0f; e.radius = 0f; e.padding = 2f; break;
                default: e.labelColor = 0x3DFCFF; e.valueColor = 0xFF7AF5; e.bg = 0x05020F; e.bgOpacity = 0.65f; e.border = 0xFF3DF2; e.borderOpacity = 0.9f; e.radius = 3f; break;
            }
            edited.add(en.getKey());
        }
    }

    // ----- widgets -----

    private void mark() { edited.add(selected); touch(); }
    private void touch() { changed = true; }
    private boolean hit(float x, float y, float w, float h) { return mx >= x && mx <= x + w && my >= y && my <= y + h; }

    static String niceName(String id) { for (int i = 0; i < Config.IDS.length; i++) if (Config.IDS[i].equals(id)) return NAMES[i]; return id; }

    private boolean button(String id, String label, float x, float y, float w, float h, boolean on) {
        rects.put(id, new float[] { x, y, w, h });
        boolean over = hit(x, y, w, h);
        hud.fill(x, y, w, h, 2f * u, on ? 0xFF8F7CFF : over ? 0xFF3A3560 : 0xFF241F45);
        float f = 5.8f * u, tw = hud.textWidth(label, f);
        if (tw > w - 2 * u) f *= (w - 2 * u) / tw; // long names shrink to fit their button
        hud.text(label, x + (w - hud.textWidth(label, f)) / 2f, y, h, f, 0xFFFFFFFF);
        return pressed && over;
    }

    private boolean switchWidget(String id, float x, float y, boolean on) {
        float w = 14f * u, h = 6.5f * u;
        rects.put(id, new float[] { x, y, w, h });
        hud.fill(x, y, w, h, h / 2f, on ? 0xFF8F7CFF : 0xFF3A3560);
        float k = h - u;
        hud.fill(on ? x + w - k - u / 2f : x + u / 2f, y + u / 2f, k, k, k / 2f, 0xFFFFFFFF);
        return pressed && hit(x, y, w, h) ? !on : on;
    }

    private boolean toggle(String id, String label, float x, float y, float w, boolean on) {
        hud.text(label, x, y, 8f * u, fs, 0xFFFFFFFF);
        rects.put(id, new float[] { x, y, w, 8f * u });
        boolean now = switchWidget(id + ":sw", x + w - 14f * u, y + 0.75f * u, on);
        return now != on || (pressed && hit(x, y, w, 8f * u)) ? !on : on;
    }

    private float slider(String id, String label, float x, float y, float w, float v, float min, float max, float step, boolean percent) {
        String val = percent ? Math.round(v * 100) + "%" : String.valueOf(Math.round(v * 100f) / 100f);
        hud.text(label, x, y, 7f * u, fs, 0xFFFFFFFF);
        hud.text(val, x + w - hud.textWidth(val, fs), y, 7f * u, fs, 0xFFC9BFFF);
        float ty = y + 8f * u, th = 3f * u;
        rects.put(id, new float[] { x, ty - 2f * u, w, th + 4f * u });
        if (pressed && hit(x, ty - 2f * u, w, th + 4f * u)) active = id;
        if (down && id.equals(active)) {
            float f = Math.max(0f, Math.min(1f, (mx - x) / w));
            v = Math.round((min + f * (max - min)) / step) * step;
            v = Math.max(min, Math.min(max, v));
        }
        float f = (v - min) / (max - min);
        hud.fill(x, ty, w, th, th / 2f, 0xFF3A3560);
        hud.fill(x, ty, w * f, th, th / 2f, 0xFF8F7CFF);
        hud.fill(x + w * f - 2.5f * u, ty - 1.5f * u, 5f * u, th + 3f * u, 2.5f * u, 0xFFFFFFFF);
        return v;
    }

    /** A colour picker: 16 swatches plus a hue strip. Returns the new colour, or -1 when nothing was picked. */
    private int colour(String label, String id, float x, float y, float w, int current) {
        hud.text(label, x, y, 7f * u, fs, 0xFFFFFFFF);
        y += 8f * u;
        int picked = -1;
        float gap = u, sw = (w - 7 * gap) / 8f, sh = 7f * u;
        for (int i = 0; i < PALETTE.length; i++) {
            float sx = x + (i % 8) * (sw + gap), sy = y + (i / 8) * (sh + gap);
            rects.put(id + ":" + i, new float[] { sx, sy, sw, sh });
            if (PALETTE[i] == current) hud.fill(sx - u * 0.7f, sy - u * 0.7f, sw + u * 1.4f, sh + u * 1.4f, 2f * u, 0xFFFFFFFF);
            hud.fill(sx, sy, sw, sh, 1.5f * u, 0xFF000000 | PALETTE[i]);
            if (pressed && hit(sx, sy, sw, sh)) picked = PALETTE[i];
        }
        float hy = y + 2 * (sh + gap), hh = 5f * u;
        rects.put(id + ":hue", new float[] { x, hy, w, hh });
        int n = 32;
        for (int i = 0; i < n; i++) hud.fill(x + w * i / n, hy, w / n + 0.5f, hh, 0f, 0xFF000000 | hsv(i / (float) n));
        if (pressed && hit(x, hy, w, hh)) active = id + ":hue";
        if (down && (id + ":hue").equals(active)) picked = hsv(Math.max(0f, Math.min(0.999f, (mx - x) / w)));
        return picked;
    }

    static int hsv(float h) {
        float s = 0.75f, v = 1f, f = h * 6f - (float) Math.floor(h * 6f);
        float p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f)), r, g, b;
        switch (((int) Math.floor(h * 6f)) % 6) {
            case 0: r = v; g = t; b = p; break;
            case 1: r = q; g = v; b = p; break;
            case 2: r = p; g = v; b = t; break;
            case 3: r = p; g = q; b = v; break;
            case 4: r = t; g = p; b = v; break;
            default: r = v; g = p; b = q;
        }
        return (Math.round(r * 255) << 16) | (Math.round(g * 255) << 8) | Math.round(b * 255);
    }
}
