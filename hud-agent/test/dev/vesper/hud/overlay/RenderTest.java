package dev.vesper.hud.overlay;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.InputStream;

/** Draws the overlay into a PNG with a software rasteriser, to check layout and look without a GPU. */
public class RenderTest {
    static BufferedImage img;
    static Font font;

    public static void main(String[] a) throws Exception {
        try (InputStream in = new java.io.FileInputStream(a[0])) { font = new Font(in); }
        int W = 1280, H = 720;
        img = new BufferedImage(W, H, BufferedImage.TYPE_INT_ARGB);
        for (int y = 0; y < H; y++) for (int x = 0; x < W; x++) img.setRGB(x, y, y < H * 0.56 ? 0xFF7CC6FB : y < H * 0.63 ? 0xFF5CB85C : 0xFF8A6A4A);
        String json = new String(java.nio.file.Files.readAllBytes(new File(a[2]).toPath()), "UTF-8");
        Config cfg = Config.parse(json);
        Stats st = new Stats(); st.fps = 144; st.frameMs = 6.9f;
        Input in = new Input(); in.down[0] = true; in.down[5] = true; // W and right mouse button held
        long now = 42L * 60 * 1_000_000_000L;
        Hud hud = new Hud(new Hud.Sink() {
            public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) { raster(x0, y0, x1, y1, u0, v0, u1, v1, argb); }
            public boolean shapes() { return true; }
            public void shape(float a, float b, float c, float d, float e, float f, float g, float h, float r, float w, int mode, int c1, int c2) { RenderTest.shape(a, b, c, d, e, f, g, h, r, w, mode, c1, c2); }
        }, font, st, in, now);
        hud.draw(cfg, W, H);
        if (a.length > 3) hud.editChrome(W, H, "frametime", "HUD editor: drag an element, + / - to resize, Right Shift to save");
        ImageIO.write(img, "png", new File(a[1]));
        System.out.println("wrote " + a[1]);
    }

    /** Texture lookup that, like mipmapping, averages over the area one screen pixel covers when the atlas is drawn smaller. */
    static float sample(float u, float v, float texelsPerPixel) {
        int n = Math.max(1, Math.min(6, (int) Math.ceil(texelsPerPixel)));
        if (n == 1) return bilinear(u, v);
        float sum = 0;
        for (int j = 0; j < n; j++) for (int i = 0; i < n; i++)
            sum += bilinear(u + ((i + 0.5f) / n - 0.5f) * texelsPerPixel / font.texW, v + ((j + 0.5f) / n - 0.5f) * texelsPerPixel / font.texH);
        return sum / (n * n);
    }
    static float bilinear(float u, float v) {
        float fx = u * font.texW - 0.5f, fy = v * font.texH - 0.5f;
        int x0 = (int) Math.floor(fx), y0 = (int) Math.floor(fy);
        float tx = fx - x0, ty = fy - y0;
        return (tex(x0, y0) * (1 - tx) + tex(x0 + 1, y0) * tx) * (1 - ty) + (tex(x0, y0 + 1) * (1 - tx) + tex(x0 + 1, y0 + 1) * tx) * ty;
    }
    static float tex(int x, int y) { x = Math.max(0, Math.min(font.texW - 1, x)); y = Math.max(0, Math.min(font.texH - 1, y)); return (font.alpha[y * font.texW + x] & 255) / 255f; }

    /** Like a GPU: a pixel is drawn when its centre is inside the quad (so touching quads never overlap). */
    static void raster(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) {
        int px0 = Math.max(0, (int) Math.ceil(x0 - 0.5f)), py0 = Math.max(0, (int) Math.ceil(y0 - 0.5f));
        int px1 = Math.min(img.getWidth(), (int) Math.ceil(x1 - 0.5f)), py1 = Math.min(img.getHeight(), (int) Math.ceil(y1 - 0.5f));
        float ca = (argb >>> 24) / 255f;
        for (int y = py0; y < py1; y++) for (int x = px0; x < px1; x++) {
            float u = u0 + (u1 - u0) * (x + 0.5f - x0) / (x1 - x0), v = v0 + (v1 - v0) * (y + 0.5f - y0) / (y1 - y0);
            float a = ca * sample(u, v, Math.abs((u1 - u0) * font.texW / (x1 - x0)));
            int dst = img.getRGB(x, y);
            float r = ((argb >> 16) & 255) * a + ((dst >> 16) & 255) * (1 - a), g = ((argb >> 8) & 255) * a + ((dst >> 8) & 255) * (1 - a), b = (argb & 255) * a + (dst & 255) * (1 - a);
            img.setRGB(x, y, 0xFF000000 | (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b));
        }
    }

    /** The same maths as the fragment shader, per pixel. */
    static void shape(float qx0, float qy0, float qx1, float qy1, float bx, float by, float bw, float bh, float r, float w, int mode, int c1, int c2) {
        int px0 = Math.max(0, (int) Math.ceil(qx0 - 0.5f)), py0 = Math.max(0, (int) Math.ceil(qy0 - 0.5f));
        int px1 = Math.min(img.getWidth(), (int) Math.ceil(qx1 - 0.5f)), py1 = Math.min(img.getHeight(), (int) Math.ceil(qy1 - 0.5f));
        float hw = bw / 2f, hh = bh / 2f, cx = bx + hw, cy = by + hh;
        for (int y = py0; y < py1; y++) for (int x = px0; x < px1; x++) {
            float cover = cover(x + 0.5f - cx, y + 0.5f - cy, hw, hh, r, w, mode);
            if (mode == 4) { // the spark uses a screen-space derivative; take it from the neighbouring pixels
                float f0 = spark(x + 0.5f - cx, y + 0.5f - cy, hw, hh), fx = spark(x + 1.5f - cx, y + 0.5f - cy, hw, hh), fy = spark(x + 0.5f - cx, y + 1.5f - cy, hw, hh);
                float fw = Math.max(Math.abs(fx - f0) + Math.abs(fy - f0), 0.0001f);
                cover = Math.max(0f, Math.min(1f, 0.5f - f0 / fw));
            }
            if (cover <= 0f) continue;
            float t = Math.max(0f, Math.min(1f, ((x + 0.5f - qx0) / (qx1 - qx0) + (y + 0.5f - qy0) / (qy1 - qy0)) / 2f));
            float ca = ((c1 >>> 24) * (1 - t) + (c2 >>> 24) * t) / 255f * cover;
            float cr = ((c1 >> 16) & 255) * (1 - t) + ((c2 >> 16) & 255) * t, cg = ((c1 >> 8) & 255) * (1 - t) + ((c2 >> 8) & 255) * t, cb = (c1 & 255) * (1 - t) + (c2 & 255) * t;
            int dst = img.getRGB(x, y);
            float rr = cr * ca + ((dst >> 16) & 255) * (1 - ca), gg = cg * ca + ((dst >> 8) & 255) * (1 - ca), bb = cb * ca + (dst & 255) * (1 - ca);
            img.setRGB(x, y, 0xFF000000 | (Math.round(rr) << 16) | (Math.round(gg) << 8) | Math.round(bb));
        }
    }
    static float spark(float px, float py, float hw, float hh) { return (float) (Math.pow(Math.abs(px) / hw, 0.65) + Math.pow(Math.abs(py) / hh, 0.65) - 1.0); }
    static float cover(float px, float py, float hw, float hh, float r, float w, int mode) {
        if (mode == 4) return 1f;
        if (mode == 5) {
            float da = (float) Math.hypot(px, py) - hw, db = (float) Math.hypot(px - 0.7119f * hw, py + 0.4963f * hw) - 0.8225f * hw;
            return Math.max(0f, Math.min(1f, 0.5f - da)) * Math.max(0f, Math.min(1f, db + 0.5f));
        }
        float qx = Math.abs(px) - hw + r, qy = Math.abs(py) - hh + r;
        float d = (float) Math.hypot(Math.max(qx, 0f), Math.max(qy, 0f)) + Math.min(Math.max(qx, qy), 0f) - r;
        if (mode == 1) return Math.max(0f, Math.min(1f, 0.5f - d));
        if (mode == 2) return Math.max(0f, Math.min(1f, 0.5f - d)) * Math.max(0f, Math.min(1f, d + w + 0.5f));
        float t = 1f - Math.max(0f, Math.min(1f, d / Math.max(w, 0.001f)));
        return t * t;
    }
}
