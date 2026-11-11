import java.awt.*;
import java.awt.image.BufferedImage;
import java.io.*;

/** Bakes ASCII 32..126 of a TTF into the overlay's font atlas (src/.../overlay/font.bin). Run once; the result is committed. */
public class BakeFont {
    public static void main(String[] a) throws Exception {
        final int SIZE = 28, CW = 32, CH = 40, BASE = 30, PADX = 2, COLS = 16, ROWS = 6;
        Font font = Font.createFont(Font.TRUETYPE_FONT, new File(a[0])).deriveFont((float) SIZE);
        BufferedImage atlas = new BufferedImage(COLS * CW, ROWS * CH, BufferedImage.TYPE_INT_ARGB);
        int[] adv = new int[96];
        Graphics2D g = atlas.createGraphics();
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_FRACTIONALMETRICS, RenderingHints.VALUE_FRACTIONALMETRICS_ON);
        g.setFont(font);
        g.setColor(Color.WHITE);
        FontMetrics fm = g.getFontMetrics();
        for (int i = 0; i < 95; i++) {
            char c = (char) (32 + i);
            int cx = (i % COLS) * CW, cy = (i / COLS) * CH;
            adv[i] = fm.charWidth(c);
            g.drawString(String.valueOf(c), cx + PADX, cy + BASE);
        }
        // cell 95 is solid white: rectangles sample from here, so everything draws with one texture
        g.fillRect((95 % COLS) * CW, (95 / COLS) * CH, CW, CH);
        g.dispose();
        try (DataOutputStream o = new DataOutputStream(new BufferedOutputStream(new FileOutputStream(a[1])))) {
            o.writeInt(0x56464e54); // "VFNT"
            o.writeShort(SIZE); o.writeShort(CW); o.writeShort(CH); o.writeShort(BASE); o.writeShort(PADX); o.writeShort(COLS); o.writeShort(ROWS);
            for (int v : adv) o.writeByte(v);
            for (int y = 0; y < atlas.getHeight(); y++) for (int x = 0; x < atlas.getWidth(); x++) o.writeByte(atlas.getRGB(x, y) >>> 24);
        }
        System.out.println("baked; first advances " + java.util.Arrays.toString(java.util.Arrays.copyOf(adv, 12)));
    }
}
