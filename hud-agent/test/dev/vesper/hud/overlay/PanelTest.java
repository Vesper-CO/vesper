package dev.vesper.hud.overlay;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.FileInputStream;
import java.util.HashSet;
import java.util.Set;

/** Clicks through the in-game panel with simulated pointer input and checks each setting changes (no GPU needed). */
public class PanelTest {
    static Font font;
    static Config cfg = Config.defaults();
    static Panel panel = new Panel();
    static Set<String> edited = new HashSet<String>();
    static final int W = 1280, H = 720;
    static BufferedImage img;
    static int fails;

    static void check(boolean ok, String what) { System.out.println((ok ? "ok    " : "FAIL  ") + what); if (!ok) fails++; }

    /** One frame: draw the HUD and the panel with the pointer at (mx,my). */
    static boolean frame(float mx, float my, boolean down, boolean pressed, boolean render) {
        if (render) { img = new BufferedImage(W, H, BufferedImage.TYPE_INT_ARGB); for (int y = 0; y < H; y++) for (int x = 0; x < W; x++) img.setRGB(x, y, y < H * 0.56 ? 0xFF7CFFFB & 0xFF7CC6FB : 0xFF5CB85C); }
        Hud hud = new Hud(new Hud.Sink() {
            public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) { if (img != null && render) RenderTest.img = img; if (render) RenderTest.raster(x0, y0, x1, y1, u0, v0, u1, v1, argb); }
        }, font, new Stats(), new Input(), 0L);
        hud.draw(cfg, W, H);
        hud.editChrome(W, H, panel.selected, null);
        return panel.frame(hud, cfg, edited, W, H, mx, my, down, pressed);
    }

    static float[] at(String id) { float[] r = panel.rects.get(id); if (r == null) throw new IllegalStateException("no widget " + id); return r; }
    static boolean click(String id) { float[] r = at(id); return frame(r[0] + r[2] / 2f, r[1] + r[3] / 2f, true, true, false); }
    static void release() { frame(0, 0, false, false, false); }
    static void render(String name) throws Exception { RenderTest.font = font; frame(-1, -1, false, false, true); ImageIO.write(img, "png", new File(name)); }

    public static void main(String[] a) throws Exception {
        try (FileInputStream in = new FileInputStream(a[0])) { font = new Font(in); }
        RenderTest.font = font;
        frame(-1, -1, false, false, false); // lay out once so the widgets have rectangles

        // tabs
        click("tab:2"); release(); frame(-1, -1, false, false, false);
        check(panel.tab == 2, "Background tab opens");

        // background: fully transparent
        panel.selected = "fps";
        frame(-1, -1, false, false, false);
        boolean ch = click("clear"); release();
        check(ch && cfg.els.get("fps").bgOpacity == 0f && edited.contains("fps"), "Fully transparent sets background opacity to 0 and marks FPS as edited");

        // slider drag: opacity from the middle to the far right
        frame(-1, -1, false, false, false);
        float[] s = at("bgop");
        frame(s[0] + s[2] * 0.5f, s[1] + s[3] / 2f, true, true, false);
        float mid = cfg.els.get("fps").bgOpacity;
        frame(s[0] + s[2] + 40, s[1] + s[3] / 2f, true, false, false); release();
        check(Math.abs(mid - 0.5f) < 0.03f && cfg.els.get("fps").bgOpacity == 1f, "Opacity slider follows a drag (50% then 100% past the end)");

        // border tab: roundness
        click("tab:3"); release(); frame(-1, -1, false, false, false);
        s = at("rad");
        frame(s[0] + 1, s[1] + s[3] / 2f, true, true, false); release();
        check(cfg.els.get("fps").radius == 0f, "Corner roundness slider reaches 0");
        frame(s[0] + s[2] - 1, s[1] + s[3] / 2f, true, true, false); release();
        check(cfg.els.get("fps").radius == 12f, "Corner roundness slider reaches 12");

        // text tab: red label colour, rainbow
        click("tab:1"); release(); frame(-1, -1, false, false, false);
        click("label:4"); release();
        check(cfg.els.get("fps").labelColor == 0xFF4D5A, "Red swatch sets the label colour");
        float[] hue = at("value:hue");
        frame(hue[0] + hue[2] * 0.5f, hue[1] + hue[3] / 2f, true, true, false); release();
        check(cfg.els.get("fps").valueColor != 0xFFFFFF && (cfg.els.get("fps").valueColor & 0xFF) > 0, "Hue strip picks a colour");
        click("rainbow"); release();
        check(cfg.els.get("fps").rainbow, "Rainbow toggles on");
        click("shadow"); release();
        check(!cfg.els.get("fps").shadow, "Shadow toggles off");

        // elements tab: hide CPS, select Clock, preset Neon for all
        click("tab:0"); release(); frame(-1, -1, false, false, false);
        boolean cpsBefore = cfg.els.get("cps").enabled;
        click("show:cps"); release();
        check(cfg.els.get("cps").enabled != cpsBefore, "The switch hides and shows an element");
        click("row:clock"); release();
        check(panel.selected.equals("clock"), "Clicking a row selects that element");
        edited.clear();
        click("preset:4"); release();
        boolean all = true;
        for (String id : Config.IDS) all &= cfg.els.get(id).border == 0xFF3DF2 && edited.contains(id);
        check(all, "Neon preset restyles every element");

        // use for all / reset
        panel.selected = "frametime"; Config.El ft = cfg.els.get("frametime"); ft.radius = 9f; ft.bgOpacity = 0.2f;
        frame(-1, -1, false, false, false);
        click("all"); release();
        check(cfg.els.get("memory").radius == 9f && cfg.els.get("memory").bgOpacity == 0.2f, "Use for all copies the look");
        click("reset"); release();
        check(ft.radius == Config.defaults().els.get("frametime").radius, "Reset restores the element's default look");

        // the panel's rectangles: nothing runs off the screen at 1080p either
        check(panel.px >= 0 && panel.px + panel.pw <= W && panel.py + panel.ph <= H, "Panel fits on the screen");

        // pictures of each tab for a human to look at
        cfg = Config.defaults(); panel = new Panel(); edited.clear();
        for (int t = 0; t < 4; t++) { panel.tab = t; render("out/panel-" + t + ".png"); }
        System.out.println(fails == 0 ? "PASS" : "FAIL (" + fails + ")");
        System.exit(fails == 0 ? 0 : 1);
    }
}
