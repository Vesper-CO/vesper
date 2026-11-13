package dev.vesper.hud.overlay;

import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.text.SimpleDateFormat;
import java.util.Date;

/** Messages go to the game's console (the launcher shows it) and to vesper-hud.log in the game folder. */
final class Log {
    private static File file;

    static void init(File f) {
        file = f;
        try { if (f.isFile() && f.length() > 200_000) f.delete(); } catch (RuntimeException ignored) { }
    }

    static synchronized void info(String m) { out("[Vesper HUD] " + m, false); }

    static synchronized void error(String m, Throwable t) {
        StringWriter w = new StringWriter();
        if (t != null) t.printStackTrace(new PrintWriter(w));
        out("[Vesper HUD] " + m + (t != null ? "\n" + w : ""), true);
    }

    private static void out(String s, boolean err) {
        (err ? System.err : System.out).println(s);
        if (file == null) return;
        try (FileWriter fw = new FileWriter(file, true)) {
            fw.write(new SimpleDateFormat("HH:mm:ss").format(new Date()) + " " + s + "\n");
        } catch (IOException ignored) { /* logging must never break the game */ }
    }
}
