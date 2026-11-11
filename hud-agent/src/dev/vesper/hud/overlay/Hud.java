package dev.vesper.hud.overlay;

/**
 * Lays out and draws every enabled element as plain quads. It knows nothing about OpenGL: a Sink receives
 * the quads (the GL surface in the game, an image in the tests). Sizes use the same "GUI units" as the
 * launcher's preview: one unit is 1/480 of the screen width, times the element's size setting.
 */
final class Hud {
    interface Sink { void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb); }

    private abstract static class Block {
        final float w, h;
        Block(float w, float h) { this.w = w; this.h = h; }
        abstract void draw(float x, float y);
    }

    private final Sink sink;
    private final Font font;
    private final Stats stats;
    private final Input input;
    private final long now;

    Hud(Sink sink, Font font, Stats stats, Input input, long now) { this.sink = sink; this.font = font; this.stats = stats; this.input = input; this.now = now; }

    void draw(Config cfg, int W, int H) {
        float unit = Math.max(0.5f, Math.min(W / 480f, H / 270f));
        for (String id : Config.IDS) {
            Config.El e = cfg.els.get(id);
            if (e == null || !e.enabled) continue;
            float su = unit * e.scale * cfg.scale;
            Block b = build(id, e, su);
            if (b == null) continue;
            b.draw(clamp01(e.x) * Math.max(0, W - b.w), clamp01(e.y) * Math.max(0, H - b.h));
        }
    }

    private static float clamp01(float v) { return Math.max(0f, Math.min(1f, v)); }

    private Block build(String id, Config.El e, float su) {
        if (id.equals("keystrokes")) return keystrokes(e, su);
        String v;
        if (id.equals("fps")) v = stats.fpsText();
        else if (id.equals("cps")) v = input.cpsText(now);
        else if (id.equals("frametime")) v = stats.frameText();
        else if (id.equals("memory")) v = stats.memoryText();
        else if (id.equals("clock")) v = stats.clockText();
        else if (id.equals("session")) v = stats.sessionText(now);
        else return null;
        return text(e, su, v);
    }

    // ----- text elements -----

    private Block text(final Config.El e, final float su, String value) {
        String t = e.template == null ? "{v}" : e.template;
        int i = t.indexOf("{v}");
        final String pre = i < 0 ? t : t.substring(0, i), post = i < 0 ? "" : t.substring(i + 3), val = i < 0 ? "" : value;
        final float fs = 8f * su, pad = e.padding * su;
        float w = font.width(pre + val + post, fs) + 2 * (pad + su), h = 9f * su + 2 * pad;
        return new Block(w, h) {
            void draw(float x, float y) {
                box(e, su, x, y, w, h);
                float bx = x + pad + su, base = baseline(y + pad, 9f * su, fs);
                label(e, pre, bx, base, fs, e.labelColor);
                bx += font.width(pre, fs);
                label(e, val, bx, base, fs, e.valueColor);
                bx += font.width(val, fs);
                label(e, post, bx, base, fs, e.labelColor);
            }
        };
    }

    /** Baseline that centres capital letters in a line box. */
    private static float baseline(float top, float lineH, float fs) { return top + lineH / 2f + 0.365f * fs; }

    private int tint(Config.El e, int rgb) {
        if (e.rainbow) return 0xFF000000 | hsv((now / 1_000_000L % 4000) / 4000f);
        return 0xFF000000 | rgb;
    }

    private void label(Config.El e, String s, float x, float base, float fs, int rgb) {
        if (s.isEmpty()) return;
        int c = tint(e, rgb);
        if (e.shadow) font.text(sink, s, x + fs / 8f, base + fs / 8f, fs, 0xFF000000 | ((c & 0xFCFCFC) >> 2));
        font.text(sink, s, x, base, fs, c);
    }

    private static int hsv(float h) {
        float s = 0.7f, v = 1f, f = h * 6f - (float) Math.floor(h * 6f);
        float p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f));
        float r, g, b;
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

    // ----- keystrokes -----

    private Block keystrokes(final Config.El e, final float su) {
        final float u = 9 * su, g = 2 * su, kh = 18 * su;
        // first column, columns spanned, row (same grid as the launcher's preview)
        final int[][] grid = { {2, 2, 0}, {0, 2, 1}, {2, 2, 1}, {4, 2, 1}, {0, 3, 2}, {3, 3, 2}, {0, 6, 3} };
        float w = 6 * u + 5 * g, h = 4 * kh + 3 * g;
        return new Block(w, h) {
            void draw(float x, float y) {
                for (int i = 0; i < grid.length; i++) {
                    float kx = x + grid[i][0] * (u + g), ky = y + grid[i][2] * (kh + g), kw = grid[i][1] * u + (grid[i][1] - 1) * g;
                    box(e, su, kx, ky, kw, kh);
                    if (input.down[i]) roundedFill(kx, ky, kw, kh, e.radius * su, 0x61000000 | e.valueColor); // 0x61 = 38% opacity
                    String lab = input.keys[i].label;
                    float fs = 7f * su;
                    label(e, lab, kx + (kw - font.width(lab, fs)) / 2f, baseline(ky, kh, fs), fs, e.valueColor);
                }
            }
        };
    }

    // ----- boxes -----

    private static int withAlpha(int rgb, float opacity) { return (Math.round(Math.max(0f, Math.min(1f, opacity)) * 255f) << 24) | (rgb & 0xFFFFFF); }

    private void box(Config.El e, float su, float x, float y, float w, float h) {
        roundedFill(x, y, w, h, e.radius * su, withAlpha(e.bg, e.bgOpacity));
        int bc = withAlpha(e.border, e.borderOpacity);
        float bw = e.borderWidth * su;
        if (e.borderWidth > 0 && (bc >>> 24) != 0) {
            float r = Math.min(e.radius * su, Math.min(w, h) / 2f);
            font.rect(sink, x + r, y, x + w - r, y + bw, bc);
            font.rect(sink, x + r, y + h - bw, x + w - r, y + h, bc);
            font.rect(sink, x, y + r, x + bw, y + h - r, bc);
            font.rect(sink, x + w - bw, y + r, x + w, y + h - r, bc);
            int n = Math.max(1, Math.min(8, (int) Math.ceil(r)));
            for (int i = 0; i < n && r > 0; i++) { // the four rounded corners, as short runs along the curve
                float y0 = r * i / n, y1 = r * (i + 1) / n, dy = r - (y0 + y1) / 2f;
                float inset = r - (float) Math.sqrt(Math.max(0, r * r - dy * dy));
                float nextDy = r - y1, nextInset = r - (float) Math.sqrt(Math.max(0, r * r - nextDy * nextDy));
                float run = Math.max(bw, nextInset - inset);
                font.rect(sink, x + inset, y + y0, x + inset + run, y + y1, bc);
                font.rect(sink, x + w - inset - run, y + y0, x + w - inset, y + y1, bc);
                font.rect(sink, x + inset, y + h - y1, x + inset + run, y + h - y0, bc);
                font.rect(sink, x + w - inset - run, y + h - y1, x + w - inset, y + h - y0, bc);
            }
        }
    }

    private void roundedFill(float x, float y, float w, float h, float r, int argb) {
        if ((argb >>> 24) == 0) return;
        r = Math.max(0f, Math.min(r, Math.min(w, h) / 2f));
        if (r < 0.5f) { font.rect(sink, x, y, x + w, y + h, argb); return; }
        font.rect(sink, x, y + r, x + w, y + h - r, argb);
        int n = Math.max(1, Math.min(8, (int) Math.ceil(r)));
        for (int i = 0; i < n; i++) {
            float y0 = r * i / n, y1 = r * (i + 1) / n, dy = r - (y0 + y1) / 2f;
            float inset = r - (float) Math.sqrt(Math.max(0, r * r - dy * dy));
            font.rect(sink, x + inset, y + y0, x + w - inset, y + y1, argb);
            font.rect(sink, x + inset, y + h - y1, x + w - inset, y + h - y0, argb);
        }
    }
}
