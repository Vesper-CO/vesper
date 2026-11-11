package dev.vesper.hud.overlay;

import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;

/** The baked font atlas (font.bin): glyph metrics, plus the texels that the GL surface uploads. */
final class Font {
    final int size, cellW, cellH, base, padX, cols, rows;
    final int[] adv = new int[96];
    final byte[] alpha; // cols*cellW by rows*cellH, one byte per texel
    final int texW, texH;

    Font(InputStream in) throws IOException {
        DataInputStream d = new DataInputStream(in);
        if (d.readInt() != 0x56464e54) throw new IOException("not a font atlas");
        size = d.readUnsignedShort(); cellW = d.readUnsignedShort(); cellH = d.readUnsignedShort(); base = d.readUnsignedShort();
        padX = d.readUnsignedShort(); cols = d.readUnsignedShort(); rows = d.readUnsignedShort();
        for (int i = 0; i < 96; i++) adv[i] = d.readUnsignedByte();
        texW = cols * cellW; texH = rows * cellH;
        alpha = new byte[texW * texH];
        d.readFully(alpha);
    }

    /** Width of a string in pixels when the font is drawn at `fs` pixels tall. */
    float width(String s, float fs) {
        float k = fs / size, w = 0;
        for (int i = 0; i < s.length(); i++) w += adv[idx(s.charAt(i))] * k;
        return w;
    }

    static int idx(char c) { return c >= 32 && c <= 126 ? c - 32 : '?' - 32; }

    /** Draw text with its baseline at `baseY`. */
    void text(Hud.Sink s, String str, float x, float baseY, float fs, int argb) {
        float k = fs / size;
        for (int i = 0; i < str.length(); i++) {
            int g = idx(str.charAt(i));
            if (g != 0) { // the space has nothing to draw
                float cx = (g % cols) * cellW, cy = (g / cols) * cellH;
                s.quad(x - padX * k, baseY - base * k, x - padX * k + cellW * k, baseY - base * k + cellH * k,
                    cx / texW, cy / texH, (cx + cellW) / texW, (cy + cellH) / texH, argb);
            }
            x += adv[g] * k;
        }
    }

    /** A solid rectangle: sample the middle of the all-white cell. */
    void rect(Hud.Sink s, float x0, float y0, float x1, float y1, int argb) {
        if ((argb >>> 24) == 0 || x1 <= x0 || y1 <= y0) return;
        int g = 95;
        float u = ((g % cols) * cellW + cellW / 2f) / texW, v = ((g / cols) * cellH + cellH / 2f) / texH;
        s.quad(x0, y0, x1, y1, u, v, u, v, argb);
    }
}
