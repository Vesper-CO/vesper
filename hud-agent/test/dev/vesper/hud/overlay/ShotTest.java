package dev.vesper.hud.overlay;

import javax.imageio.ImageIO;
import java.awt.image.BufferedImage;
import java.io.File;
import java.io.FileInputStream;
import java.util.HashSet;

/** Draws the Vesper pause-menu button and the editor on top of a real screenshot of the game, to check they line up. */
public class ShotTest {
    public static void main(String[] a) throws Exception {
        Font font; try (FileInputStream in = new FileInputStream(a[0])) { font = new Font(in); }
        RenderTest.font = font;
        BufferedImage shot = ImageIO.read(new File(a[1]));
        int W = shot.getWidth(), H = shot.getHeight();
        int s = PauseMenu.guiScale(W, H, 0);
        System.out.println("gui scale " + s + " button " + java.util.Arrays.toString(PauseMenu.button(W, H, s, true)) + " freeX " + PauseMenu.freeX(W, s));
        for (int mode = 0; mode < 3; mode++) {
            BufferedImage img = new BufferedImage(W, H, BufferedImage.TYPE_INT_ARGB);
            img.getGraphics().drawImage(shot, 0, 0, null);
            if (mode == 2) { // edit mode: hide the old editor that is in the screenshot
                for (int y = 30; y < 680; y++) for (int x = 1300; x < W; x++) img.setRGB(x, y, shot.getRGB(x - 1280, y));
            }
            RenderTest.img = img;
            Hud hud = new Hud(new Hud.Sink() { public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) { RenderTest.raster(x0, y0, x1, y1, u0, v0, u1, v1, argb); } }, font, new Stats(), new Input(), 0L);
            hud.unit = Math.max(0.5f, Math.min(W / 480f, H / 270f));
            if (mode < 2) {
                float[] r = PauseMenu.button(W, H, s, true);
                hud.pauseButton(r[0], r[1], r[2], mode == 1);
                if (mode == 1) hud.tooltip("Vesper HUD editor", r[0] + r[2] / 2f, r[1] + r[2] + 3f * s, s, W, H);
            } else {
                Config cfg = Config.defaults();
                hud.draw(cfg, W, H);
                Panel p = new Panel(); p.tab = 2; p.selected = "fps"; p.minX = PauseMenu.freeX(W, s);
                hud.editChrome(W, H, "fps", null);
                p.frame(hud, cfg, new HashSet<String>(), W, H, -1, -1, false, false);
                hud.banner(W, H, "Drag items to move them  |  + and - resize");
                System.out.println("panel x " + p.px + ".." + (p.px + p.pw) + " (menu ends at " + PauseMenu.freeX(W, s) + ")");
            }
            ImageIO.write(img, "png", new File("out/shot-" + mode + ".png"));
        }
    }
}
