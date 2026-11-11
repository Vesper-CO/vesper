package dev.vesper.hud.overlay;

import java.util.Locale;

/** Numbers the overlay can measure without looking inside the game: frame rate, frame time, uptime, memory. */
final class Stats {
    private final long start = System.nanoTime();
    private long last, windowStart;
    private int frames;
    int fps;
    float frameMs;

    /** Call once per presented frame. */
    void frame(long now) {
        if (last != 0) frameMs = frameMs == 0 ? (now - last) / 1e6f : frameMs * 0.9f + ((now - last) / 1e6f) * 0.1f;
        last = now;
        if (windowStart == 0) windowStart = now;
        frames++;
        if (now - windowStart >= 500_000_000L) {
            fps = Math.round(frames * 1e9f / (now - windowStart));
            frames = 0; windowStart = now;
        }
    }

    String fpsText() { return String.valueOf(fps); }
    String frameText() { return String.format(Locale.ROOT, "%.1f ms", frameMs); }

    String memoryText() {
        Runtime r = Runtime.getRuntime();
        return String.format(Locale.ROOT, "%.1f / %.1f GB", (r.totalMemory() - r.freeMemory()) / 1073741824.0, r.maxMemory() / 1073741824.0);
    }

    String sessionText(long now) {
        long s = (now - start) / 1_000_000_000L;
        return s >= 3600 ? String.format(Locale.ROOT, "%d:%02d:%02d", s / 3600, (s / 60) % 60, s % 60) : String.format(Locale.ROOT, "%d:%02d", s / 60, s % 60);
    }

    String clockText() {
        java.util.Calendar c = java.util.Calendar.getInstance();
        return String.format(Locale.ROOT, "%02d:%02d", c.get(java.util.Calendar.HOUR_OF_DAY), c.get(java.util.Calendar.MINUTE));
    }
}
