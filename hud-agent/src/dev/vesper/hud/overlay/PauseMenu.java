package dev.vesper.hud.overlay;

/**
 * Where the game draws its pause menu, worked out from the window size and the player's GUI scale, so the
 * Vesper button can sit next to the game's own buttons without touching the game. The pause menu is a column of
 * 204-wide rows, 20 high with 4 between, laid out the same way in every version from 1.14 to 26.x: the first row
 * ("Back to Game") starts 50 units below the top of a block that is placed a quarter of the way down the screen.
 * In 26.x the third row is a strip of small square icon buttons (bug report, feedback, ...); the Vesper button is
 * added to the end of that strip. Older versions have no strip, so it goes beside "Back to Game". All numbers
 * are in the game's GUI units; the methods return pixels.
 */
final class PauseMenu {
    static final int ROW = 24, BLOCK_H = 166, FIRST_PAD = 50, WIDE = 204, STRIP_W = 92;

    /** The game's own GUI scale: the biggest whole number that keeps the screen at least 320x240 units. */
    static int guiScale(int W, int H, int option) {
        int s = 1;
        while (s != option && s < W && s < H && W / (s + 1) >= 320 && H / (s + 1) >= 240) s++;
        return s;
    }

    static int guiW(int W, int s) { return (int) Math.ceil(W / (double) s); }
    static int guiH(int H, int s) { return (int) Math.ceil(H / (double) s); }

    /** Top of the "Back to Game" row, in GUI units. */
    static int top(int H, int s) { return (int) ((guiH(H, s) - BLOCK_H) * 0.25f) + FIRST_PAD; }

    /** x, y, size of the Vesper button in pixels. */
    static float[] button(int W, int H, int s, boolean iconStrip) {
        int gw = guiW(W, s), x, y;
        if (iconStrip) { x = (gw - STRIP_W) / 2 + STRIP_W + 4; y = top(H, s) + 2 * ROW; }
        else { x = (gw - WIDE) / 2 + WIDE + 4; y = top(H, s); }
        return new float[] { x * s, y * s, 20f * s };
    }

    /** Pixel x where the free space to the right of the game's buttons starts (the editor panel lives there). */
    static float freeX(int W, int s) { return ((guiW(W, s) - WIDE) / 2 + WIDE + 4) * s; }

    /** The block of game buttons (pixels): a click here may open another screen. */
    static float[] block(int W, int H, int s) {
        int gw = guiW(W, s), left = (gw - WIDE) / 2 - 4, t = top(H, s);
        return new float[] { left * s, t * s, (WIDE + 8) * s, (4 * ROW + 20) * s };
    }

    static boolean inside(float[] r, float px, float py, float size) { return px >= r[0] && px <= r[0] + size && py >= r[1] && py <= r[1] + size; }
}
