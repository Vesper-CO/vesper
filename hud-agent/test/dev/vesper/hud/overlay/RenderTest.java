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
        new Hud(new Hud.Sink() {
            public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) { raster(x0, y0, x1, y1, u0, v0, u1, v1, argb); }
        }, font, st, in, now).draw(cfg, W, H);
        ImageIO.write(img, "png", new File(a[1]));
        System.out.println("wrote " + a[1]);
    }

    static float sample(float u, float v) {
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
            float a = ca * sample(u, v);
            int dst = img.getRGB(x, y);
            float r = ((argb >> 16) & 255) * a + ((dst >> 16) & 255) * (1 - a), g = ((argb >> 8) & 255) * a + ((dst >> 8) & 255) * (1 - a), b = (argb & 255) * a + (dst & 255) * (1 - a);
            img.setRGB(x, y, 0xFF000000 | (Math.round(r) << 16) | (Math.round(g) << 8) | Math.round(b));
        }
    }
}
