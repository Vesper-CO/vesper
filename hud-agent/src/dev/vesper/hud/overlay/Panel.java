package dev.vesper.hud.overlay;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * The in-game HUD editor menu, drawn over the game in edit mode and styled like the launcher (night glass, one purple
 * accent). It is split into four small tabs so only a few controls are on screen at once:
 *
 *   Elements  what to show (a switch per element, plus one for the whole HUD) and the size of the selected one
 *   Looks     one-click presets, each drawn as a live sample of what it will look like
 *   Colors    ONE colour picker for whichever part you choose: label, value, fill or border (plus opacity)
 *   Shape     border width, corner roundness, padding
 *
 * "All" in the Editing row makes every change apply to every element at once, so most people never need to select
 * anything. It is immediate-mode: every frame it draws itself and reports clicks, so it needs no state beyond
 * the tab, the selected element and the colour part being edited.
 */
final class Panel {
    static final String[] TABS = { "Elements", "Looks", "Colors", "Shape" };
    static final String[] TARGETS = { "Label", "Value", "Fill", "Border" };
    static final int[] PALETTE = {
        0xFFFFFF, 0xC8CCD8, 0x7A7F90, 0x0B0B17, 0xFF4D5A, 0xFFA020, 0xFFE27A, 0xA4E84A,
        0x3DDC84, 0x3DFCFF, 0x5AA8FF, 0x3A6BFF, 0x8F7CFF, 0xFF5CE1, 0xFFA6C9, 0x8A6038 };
    static final String[] NAMES = { "FPS", "CPS", "Frame time", "Memory", "Clock", "Playtime", "Keystrokes" };
    static final String[] PRESET_NAMES = { "Vesper Night", "Vesper Day", "Glass", "Transparent", "Neon" };

    // the launcher's palette, as ARGB
    private static final int PANEL = 0xF5100E26, LINE = 0x55AFA2FF, TEXT = 0xFFECEBF7, MUTED = 0xFFA09FBA,
        ACCENT = 0xFF8F7CFF, ACCENT2 = 0xFFC9BFFF, SOFT = 0x428F7CFF, CTRL = 0xFF211E42, CTRL_HOVER = 0xFF302B63, TRACK = 0xFF38336A;

    int tab, target;
    String selected = "fps";
    /** Every change applies to every element (the "All" chip). */
    boolean all;
    /** Set when the player presses Done or the close button. */
    boolean done;
    /** The panel never starts left of this pixel column (the game's own buttons are there). */
    float minX;
    /** widget id -> x, y, w, h in pixels (hit areas; the tests click through these) */
    final Map<String, float[]> rects = new LinkedHashMap<String, float[]>();
    float px, py, pw, ph;

    private Hud hud;
    private float u, mx, my, fs;
    private boolean down, pressed;
    private String active; // the slider or colour strip being dragged
    private boolean changed;
    private Config cfg;
    private Config.El cur;
    private Set<String> edited;

    boolean contains(float x, float y) { return x >= px - 2f * u && x <= px + pw + 2f * u && y >= py - 2f * u && y <= py + ph + 2f * u; }

    /** Draw the panel and handle the pointer. Returns true when a setting changed. */
    boolean frame(Hud hud, Config cfg, Set<String> edited, int W, int H, float mx, float my, boolean down, boolean pressed) {
        this.hud = hud; this.cfg = cfg; this.edited = edited; this.mx = mx; this.my = my; this.down = down; this.pressed = pressed;
        if (!down) active = null;
        changed = false;
        rects.clear();
        u = hud.unit; fs = 6.2f * u;
        float wu = Math.max(118f, Math.min(160f, (W - minX - 8f * u) / u));
        pw = wu * u; ph = 170f * u; px = W - pw - 4f * u; py = 6f * u;
        cur = cfg.els.get(selected);
        if (cur == null) { selected = "fps"; cur = cfg.els.get("fps"); }

        // card: halo, glass body, thin outline, accent line along the top
        hud.glow(px, py, pw, ph, 6f * u, 4f * u, 0x8F7CFF, 0x30);
        hud.fill(px, py, pw, ph, 6f * u, PANEL);
        hud.ring(px, py, pw, ph, 6f * u, 0.8f * u, LINE);
        hud.fill(px + 8f * u, py, pw - 16f * u, 1.2f * u, 0f, ACCENT);

        float pad = 7f * u, x = px + pad, w = pw - 2 * pad, y = py + 7f * u;
        header(x, y, w);
        y += 18f * u;

        // tabs: a dark track with the open one lit
        hud.fill(x, y, w, 12.5f * u, 5f * u, 0x66070614);
        float tw = (w - 3f * u) / 4f;
        for (int i = 0; i < 4; i++) if (tabButton("tab:" + i, TABS[i], x + 0.75f * u + i * (tw + 0.5f * u), y + 0.75f * u, tw, 11f * u, i == tab)) tab = i;
        y += 17f * u;

        if (tab == 0) elementsTab(x, y, w);
        else if (tab == 1) looksTab(x, y, w);
        else if (tab == 2) colorsTab(x, y, w);
        else shapeTab(x, y, w);

        // footer
        float fy = py + ph - 17f * u, rw = w * 0.36f;
        if (button("reset", all ? "Reset all" : "Reset", x, fy, rw, 11f * u, 0)) {
            for (Map.Entry<String, Config.El> e : cfg.els.entrySet()) {
                if (!all && e.getValue() != cur) continue;
                Config.El d = Config.defaults().els.get(e.getKey());
                if (d != null) { e.getValue().copyStyle(d, true); edited.add(e.getKey()); }
            }
            touch();
        }
        if (button("done", "Done", x + rw + 3f * u, fy, w - rw - 3f * u, 11f * u, 2)) done = true;
        return changed;
    }

    // ----- header and the "Editing" row -----

    private void header(float x, float y, float w) {
        hud.logo(x, y, 13f * u, false);
        fit("HUD editor", x + 17f * u, y - 1f * u, w - 34f * u, 8f * u, 8.4f * u, TEXT);
        fit("Drag items on screen to move them", x + 17f * u, y + 7f * u, w - 31f * u, 6f * u, 5.1f * u, MUTED);
        float bx = x + w - 11f * u;
        boolean over = hit(bx, y, 11f * u, 11f * u);
        rects.put("close", new float[] { bx, y, 11f * u, 11f * u });
        hud.fill(bx, y, 11f * u, 11f * u, 4f * u, over ? CTRL_HOVER : CTRL);
        float xs = 7f * u, xw = hud.textWidth("x", xs);
        hud.text("x", bx + (11f * u - xw) / 2f, y, 11f * u, xs, over ? TEXT : MUTED);
        if (pressed && over) done = true;
    }

    /** Which element the next change goes to: arrows to pick another, or "All" for every element. */
    private float editingRow(float x, float y, float w) {
        float h = 11f * u, chip = 20f * u;
        float nw = w - chip - 2f * u;
        hud.fill(x, y, nw, h, 4f * u, 0x44070614);
        if (all) {
            fit("Editing every element", x + 4f * u, y, nw - 8f * u, h, fs, ACCENT2);
        } else {
            if (button("prev", "<", x + 1f * u, y + 1f * u, 9f * u, h - 2f * u, 0)) step(-1);
            if (button("next", ">", x + nw - 10f * u, y + 1f * u, 9f * u, h - 2f * u, 0)) step(1);
            String n = "Editing: " + niceName(selected);
            float ntw = Math.min(hud.textWidth(n, fs), nw - 24f * u);
            fit(n, x + 12f * u + (nw - 24f * u - ntw) / 2f, y, nw - 24f * u, h, fs, TEXT);
        }
        if (button("all", "All", x + w - chip, y, chip, h, all ? 1 : 0)) all = !all;
        return y + h + 4f * u;
    }

    private void step(int d) {
        int i = 0;
        for (int k = 0; k < Config.IDS.length; k++) if (Config.IDS[k].equals(selected)) i = k;
        selected = Config.IDS[(i + d + Config.IDS.length) % Config.IDS.length];
    }

    // ----- tabs -----

    private void elementsTab(float x, float y, float w) {
        // the whole HUD
        float rh = 10.5f * u;
        hud.fill(x, y, w, rh, 4f * u, 0x33070614);
        hud.text("Show the HUD", x + 4f * u, y, rh, fs, TEXT);
        rects.put("master", new float[] { x, y, w, rh });
        boolean m = switchWidget("master:sw", x + w - 17f * u, y + 1.5f * u, cfg.enabled);
        if (m != cfg.enabled) { cfg.enabled = m; touch(); }
        y += rh + 4f * u;

        for (int i = 0; i < Config.IDS.length; i++) {
            String id = Config.IDS[i];
            Config.El e = cfg.els.get(id);
            boolean on = id.equals(selected);
            rh = 10f * u;
            rects.put("row:" + id, new float[] { x, y, w - 20f * u, rh });
            boolean over = hit(x, y, w - 20f * u, rh);
            hud.fill(x, y, w, rh, 4f * u, on ? SOFT : over ? 0x26AFA2FF : 0x00000000);
            if (on) hud.fill(x, y + 1.5f * u, 1.6f * u, rh - 3f * u, 0.8f * u, ACCENT);
            hud.text(niceName(id), x + 5f * u, y, rh, fs, e.enabled ? TEXT : MUTED);
            if (pressed && over) selected = id;
            boolean now = switchWidget("show:" + id, x + w - 17f * u, y + 1.75f * u, e.enabled);
            if (now != e.enabled) { e.enabled = now; edited.add(id); touch(); }
            y += 10.8f * u;
        }
        y += 3f * u;
        float v = slider("size", "Size of " + niceName(selected), x, y, w, cur.scale, 0.5f, 3f, 0.05f, false);
        if (v != cur.scale) { cur.scale = v; mark(true); }
    }

    private void looksTab(float x, float y, float w) {
        fit("Pick a look for every element", x, y - 1f * u, w, 8f * u, fs, MUTED);
        y += 9f * u;
        float gap = 3f * u, tw = (w - gap) / 2f, th = 26f * u;
        for (int i = 0; i < PRESET_NAMES.length; i++) {
            float bx = x + (i % 2) * (tw + gap), by = y + (i / 2) * (th + gap);
            rects.put("preset:" + i, new float[] { bx, by, tw, th });
            boolean over = hit(bx, by, tw, th), on = matches(cur, i);
            hud.fill(bx, by, tw, th, 5f * u, over ? CTRL_HOVER : CTRL);
            // a small world behind the sample, so transparent and glass looks can be judged
            hud.fill(bx + 1.5f * u, by + 1.5f * u, tw - 3f * u, th - 12f * u, 3.5f * u, 0xFF3F78B8);
            hud.fill(bx + 1.5f * u, by + 1.5f * u + (th - 12f * u) * 0.62f, tw - 3f * u, (th - 12f * u) * 0.38f, 0f, 0xFF4C8A44);
            Config.El s = Config.defaults().els.get("fps");
            s.enabled = true; styleFor(i, s);
            hud.sample(s, "144", bx + tw / 2f, by + 1.5f * u + (th - 12f * u) / 2f, 0.62f * u);
            fit(PRESET_NAMES[i], bx + 2f * u, by + th - 10f * u, tw - 4f * u, 9f * u, 5.6f * u, on ? ACCENT2 : TEXT);
            if (on) hud.ring(bx, by, tw, th, 5f * u, 1f * u, ACCENT);
            else if (over) hud.ring(bx, by, tw, th, 5f * u, 0.8f * u, 0x888F7CFF);
            if (pressed && over) { applyPreset(i); touch(); }
        }
    }

    private void colorsTab(float x, float y, float w) {
        y = editingRow(x, y, w);
        // which part to colour
        float tw = (w - 3f * u) / 4f;
        for (int i = 0; i < 4; i++) if (button("target:" + i, TARGETS[i], x + i * (tw + u), y, tw, 10f * u, i == target ? 1 : 0)) target = i;
        y += 14f * u;

        int c = target == 0 ? cur.labelColor : target == 1 ? cur.valueColor : target == 2 ? cur.bg : cur.border;
        int picked = picker(x, y, w, c);
        if (picked >= 0) {
            if (target == 0) cur.labelColor = picked; else if (target == 1) cur.valueColor = picked; else if (target == 2) cur.bg = picked; else cur.border = picked;
            mark(false);
        }
        y += 25f * u;
        // current colour and its code
        hud.fill(x, y, 9f * u, 7f * u, 2f * u, 0xFF000000 | c);
        hud.ring(x, y, 9f * u, 7f * u, 2f * u, 0.7f * u, 0x88FFFFFF);
        hud.text(Config.hexOf(c).toUpperCase(), x + 12f * u, y, 7f * u, fs, MUTED);
        y += 11f * u;

        if (target <= 1) {
            boolean r = toggle("rainbow", "Rainbow text", x, y, w, cur.rainbow); if (r != cur.rainbow) { cur.rainbow = r; mark(false); }
            y += 12f * u;
            boolean s = toggle("shadow", "Text shadow", x, y, w, cur.shadow); if (s != cur.shadow) { cur.shadow = s; mark(false); }
        } else {
            boolean fill = target == 2;
            float op = fill ? cur.bgOpacity : cur.borderOpacity;
            float v = slider("opacity", fill ? "Fill opacity" : "Border opacity", x, y, w, op, 0f, 1f, 0.01f, true);
            if (v != op) { if (fill) cur.bgOpacity = v; else cur.borderOpacity = v; mark(false); }
            y += 17f * u;
            float bw = (w - u) / 2f;
            if (button("clear", fill ? "Transparent" : "No border", x, y, bw, 10f * u, op == 0f ? 1 : 0)) { if (fill) cur.bgOpacity = 0f; else cur.borderOpacity = 0f; mark(false); }
            if (button("solid", "Solid", x + bw + u, y, bw, 10f * u, op == 1f ? 1 : 0)) { if (fill) cur.bgOpacity = 1f; else cur.borderOpacity = 1f; mark(false); }
        }
    }

    private void shapeTab(float x, float y, float w) {
        y = editingRow(x, y, w);
        y += 2f * u;
        float v = slider("bw", "Border width", x, y, w, cur.borderWidth, 0f, 4f, 0.5f, false); if (v != cur.borderWidth) { cur.borderWidth = v; mark(false); }
        y += 17f * u;
        v = slider("rad", "Corner roundness", x, y, w, cur.radius, 0f, 12f, 1f, false); if (v != cur.radius) { cur.radius = v; mark(false); }
        y += 17f * u;
        v = slider("pad", "Padding", x, y, w, cur.padding, 0f, 12f, 1f, false); if (v != cur.padding) { cur.padding = v; mark(false); }
        y += 22f * u;
        fit("Tip: hover an element and press + or - to resize it.", x, y, w, 7f * u, 5f * u, MUTED);
    }

    // ----- presets -----

    /** The colours and shape of preset i, applied to one element (never its place or whether it is on). */
    static void styleFor(int i, Config.El e) {
        e.labelColor = 0xC9BFFF; e.valueColor = 0xFFFFFF; e.rainbow = false; e.shadow = true;
        e.bg = 0x0B0B17; e.bgOpacity = 0.55f; e.border = 0x8F7CFF; e.borderOpacity = 0.35f; e.borderWidth = 1f; e.radius = 4f; e.padding = 4f;
        switch (i) {
            case 0: e.radius = 5f; break;
            case 1: e.labelColor = 0x1F64D6; e.valueColor = 0x14213D; e.shadow = false; e.bg = 0xFFFFFF; e.bgOpacity = 0.62f; e.border = 0x1F64D6; e.radius = 5f; break;
            case 2: e.labelColor = 0xE8EEFF; e.bg = 0xFFFFFF; e.bgOpacity = 0.12f; e.border = 0xFFFFFF; e.borderOpacity = 0.4f; e.radius = 7f; break;
            case 3: e.labelColor = 0xFFFFFF; e.bgOpacity = 0f; e.borderOpacity = 0f; e.radius = 0f; e.padding = 2f; break;
            default: e.labelColor = 0x3DFCFF; e.valueColor = 0xFF7AF5; e.bg = 0x05020F; e.bgOpacity = 0.65f; e.border = 0xFF3DF2; e.borderOpacity = 0.9f; e.radius = 3f; break;
        }
    }

    private static boolean matches(Config.El e, int i) {
        Config.El p = new Config.El(true, 0, 0, "");
        styleFor(i, p);
        return e.labelColor == p.labelColor && e.valueColor == p.valueColor && e.bg == p.bg && e.border == p.border && e.rainbow == p.rainbow && e.shadow == p.shadow
            && Math.abs(e.bgOpacity - p.bgOpacity) < 0.005f && Math.abs(e.borderOpacity - p.borderOpacity) < 0.005f && Math.abs(e.radius - p.radius) <= 1.5f && e.padding == p.padding && e.borderWidth == p.borderWidth;
    }

    /** A preset restyles every element: it is the quick way to change the whole HUD. */
    private void applyPreset(int i) {
        for (Map.Entry<String, Config.El> en : cfg.els.entrySet()) { styleFor(i, en.getValue()); edited.add(en.getKey()); }
    }

    // ----- change bookkeeping -----

    /** A setting of the selected element changed; with "All" on, every element gets the same look (and size when asked). */
    private void mark(boolean withScale) {
        edited.add(selected);
        if (all) for (Map.Entry<String, Config.El> e : cfg.els.entrySet()) if (e.getValue() != cur) { e.getValue().copyStyle(cur, withScale); edited.add(e.getKey()); }
        touch();
    }
    private void touch() { changed = true; }
    private boolean hit(float x, float y, float w, float h) { return mx >= x && mx <= x + w && my >= y && my <= y + h; }

    static String niceName(String id) { for (int i = 0; i < Config.IDS.length; i++) if (Config.IDS[i].equals(id)) return NAMES[i]; return id; }

    // ----- widgets -----

    /** Text that shrinks to fit a width, vertically centred in a box of height h. */
    private void fit(String s, float x, float y, float maxW, float h, float size, int argb) {
        float tw = hud.textWidth(s, size);
        if (tw > maxW && tw > 0) size *= maxW / tw;
        hud.text(s, x, y, h, size, argb);
    }

    /** style: 0 normal, 1 selected, 2 primary (the Vesper purple, with a glow). */
    private boolean button(String id, String label, float x, float y, float w, float h, int style) {
        rects.put(id, new float[] { x, y, w, h });
        boolean over = hit(x, y, w, h);
        float r = Math.min(4f * u, h / 2f);
        if (style == 2) {
            hud.glow(x, y, w, h, r, over ? 3f * u : 2f * u, 0x6F5CF2, over ? 0x70 : 0x40);
            hud.fill(x, y, w, h, r, over ? 0xFFA597FF : ACCENT);
            hud.fillTop(x + 0.5f * u, y + 0.5f * u, w - u, h * 0.45f, r, 0x38FFFFFF);
        } else {
            hud.fill(x, y, w, h, r, style == 1 ? SOFT : over ? CTRL_HOVER : CTRL);
            if (style == 1) hud.ring(x, y, w, h, r, 0.8f * u, 0xAA8F7CFF);
        }
        float f = 5.9f * u, tw = hud.textWidth(label, f);
        if (tw > w - 2 * u) f *= (w - 2 * u) / tw; // long names shrink to fit their button
        hud.text(label, x + (w - hud.textWidth(label, f)) / 2f, y, h, f, style == 2 ? 0xFF0D0B1F : style == 1 ? ACCENT2 : TEXT);
        return pressed && over;
    }

    private boolean tabButton(String id, String label, float x, float y, float w, float h, boolean on) {
        rects.put(id, new float[] { x, y, w, h });
        boolean over = hit(x, y, w, h);
        if (on) {
            hud.fill(x, y, w, h, 4.2f * u, SOFT);
            hud.ring(x, y, w, h, 4.2f * u, 0.8f * u, 0xAA8F7CFF);
        } else if (over) hud.fill(x, y, w, h, 4.2f * u, 0x26AFA2FF);
        float f = 5.7f * u, tw = hud.textWidth(label, f);
        if (tw > w - 2 * u) f *= (w - 2 * u) / tw;
        hud.text(label, x + (w - hud.textWidth(label, f)) / 2f, y, h, f, on ? ACCENT2 : over ? TEXT : MUTED);
        return pressed && over;
    }

    private boolean switchWidget(String id, float x, float y, boolean on) {
        float w = 14f * u, h = 7f * u;
        rects.put(id, new float[] { x, y, w, h });
        hud.fill(x, y, w, h, h / 2f, on ? ACCENT : TRACK);
        float k = h - 1.6f * u;
        hud.fill(on ? x + w - k - 0.8f * u : x + 0.8f * u, y + 0.8f * u, k, k, k / 2f, 0xFFFFFFFF);
        return pressed && hit(x, y, w, h) ? !on : on;
    }

    private boolean toggle(String id, String label, float x, float y, float w, boolean on) {
        hud.text(label, x, y, 7f * u, fs, TEXT);
        rects.put(id, new float[] { x, y, w, 7.5f * u });
        boolean now = switchWidget(id + ":sw", x + w - 14f * u, y, on);
        return now != on || (pressed && hit(x, y, w, 7.5f * u)) ? !on : on;
    }

    private float slider(String id, String label, float x, float y, float w, float v, float min, float max, float step, boolean percent) {
        String val = percent ? Math.round(v * 100) + "%" : String.valueOf(Math.round(v * 100f) / 100f);
        float vw = hud.textWidth(val, fs);
        fit(label, x, y, w - vw - 3f * u, 7f * u, fs, TEXT);
        hud.text(val, x + w - vw, y, 7f * u, fs, ACCENT2);
        float ty = y + 9f * u, th = 3f * u;
        rects.put(id, new float[] { x, ty - 2.5f * u, w, th + 5f * u });
        if (pressed && hit(x, ty - 2.5f * u, w, th + 5f * u)) active = id;
        if (down && id.equals(active)) {
            float f = Math.max(0f, Math.min(1f, (mx - x) / w));
            v = Math.round((min + f * (max - min)) / step) * step;
            v = Math.max(min, Math.min(max, v));
        }
        float f = (v - min) / (max - min);
        hud.fill(x, ty, w, th, th / 2f, TRACK);
        hud.fill(x, ty, Math.max(th, w * f), th, th / 2f, ACCENT);
        boolean grab = id.equals(active);
        if (grab) hud.glow(x + w * f - 3f * u, ty - 1.5f * u, 6f * u, th + 3f * u, 3f * u, 2f * u, 0x8F7CFF, 0x60);
        hud.fill(x + w * f - 3f * u, ty - 1.5f * u, 6f * u, th + 3f * u, 3f * u, 0xFFFFFFFF);
        return v;
    }

    /** A colour picker: 16 swatches plus a hue strip. Returns the new colour, or -1 when nothing was picked. */
    private int picker(float x, float y, float w, int current) {
        int picked = -1;
        float gap = u, sw = (w - 7 * gap) / 8f, sh = 8f * u;
        for (int i = 0; i < PALETTE.length; i++) {
            float sx = x + (i % 8) * (sw + gap), sy = y + (i / 8) * (sh + gap);
            rects.put("pick:" + i, new float[] { sx, sy, sw, sh });
            boolean over = hit(sx, sy, sw, sh);
            if (PALETTE[i] == current) hud.fill(sx - u * 0.9f, sy - u * 0.9f, sw + u * 1.8f, sh + u * 1.8f, 3f * u, 0xFFFFFFFF);
            else if (over) hud.fill(sx - u * 0.6f, sy - u * 0.6f, sw + u * 1.2f, sh + u * 1.2f, 3f * u, 0x99FFFFFF);
            hud.fill(sx, sy, sw, sh, 2.2f * u, 0xFF000000 | PALETTE[i]);
            if (pressed && over) picked = PALETTE[i];
        }
        float hy = y + 2 * (sh + gap), hh = 5f * u;
        rects.put("pick:hue", new float[] { x, hy, w, hh });
        int n = 32;
        for (int i = 0; i < n; i++) hud.fill(x + w * i / n, hy, w / n + 0.5f, hh, 0f, 0xFF000000 | hsv(i / (float) n));
        if (pressed && hit(x, hy - u, w, hh + 2 * u)) active = "pick:hue";
        if (down && "pick:hue".equals(active)) picked = hsv(Math.max(0f, Math.min(0.999f, (mx - x) / w)));
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
