package dev.vesper.hud.overlay;

import org.lwjgl.glfw.GLFW;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayDeque;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/** Keyboard and mouse state read straight from GLFW (the library every Minecraft version uses), plus click counting. */
final class Input {
    static final class Key { String label; int code; boolean mouse; Key(String l, int c, boolean m) { label = l; code = c; mouse = m; } }

    // forward, left, back, right, attack, use, jump (in the order the keystroke grid draws them)
    final Key[] keys = {
        new Key("W", 87, false), new Key("A", 65, false), new Key("S", 83, false), new Key("D", 68, false),
        new Key("LMB", 0, true), new Key("RMB", 1, true), new Key("SPACE", 32, false),
    };
    private static final String[] OPTION = { "key_key.forward", "key_key.left", "key_key.back", "key_key.right", "key_key.attack", "key_key.use", "key_key.jump" };
    final boolean[] down = new boolean[7];
    private final ArrayDeque<Long> left = new ArrayDeque<Long>(), right = new ArrayDeque<Long>();
    private boolean wasLeft, wasRight, wasF1, wasShift, wasPlus, wasMinus;
    private long lastRead;
    boolean f1Pressed; // true for the one frame in which F1 went down
    boolean rshiftPressed, plusPressed, minusPressed; // same, for Right Shift (edit mode), + and -

    void poll(long window, long nowNanos, File options) {
        if (nowNanos - lastRead > 10_000_000_000L || lastRead == 0) { lastRead = nowNanos; readBindings(options); }
        for (int i = 0; i < keys.length; i++) {
            Key k = keys[i];
            down[i] = (k.mouse ? GLFW.glfwGetMouseButton(window, k.code) : GLFW.glfwGetKey(window, k.code)) == 1;
        }
        long ms = nowNanos / 1_000_000L;
        boolean l = GLFW.glfwGetMouseButton(window, 0) == 1, r = GLFW.glfwGetMouseButton(window, 1) == 1;
        if (l && !wasLeft) left.addLast(ms);
        if (r && !wasRight) right.addLast(ms);
        wasLeft = l; wasRight = r;
        boolean f1 = GLFW.glfwGetKey(window, 290) == 1;
        f1Pressed = f1 && !wasF1;
        wasF1 = f1;
        boolean sh = GLFW.glfwGetKey(window, 344) == 1, pl = GLFW.glfwGetKey(window, 61) == 1 || GLFW.glfwGetKey(window, 334) == 1, mi = GLFW.glfwGetKey(window, 45) == 1 || GLFW.glfwGetKey(window, 333) == 1;
        rshiftPressed = sh && !wasShift; plusPressed = pl && !wasPlus; minusPressed = mi && !wasMinus;
        wasShift = sh; wasPlus = pl; wasMinus = mi;
    }

    /** The mouse pointer is free (a menu is open) rather than captured by the game for looking around. */
    static boolean cursorFree(long window) { return GLFW.glfwGetInputMode(window, 0x33001) == 0x34001; }

    static int cps(ArrayDeque<Long> q, long nowNanos) {
        long cutoff = nowNanos / 1_000_000L - 1000;
        while (!q.isEmpty() && q.peekFirst() < cutoff) q.pollFirst();
        return q.size();
    }

    String cpsText(long nowNanos) { return cps(left, nowNanos) + " | " + cps(right, nowNanos); }

    /** Follow the player's own key bindings (options.txt), so a rebound key shows the right letter. */
    private void readBindings(File options) {
        try {
            if (options == null || !options.isFile()) return;
            List<String> lines = Files.readAllLines(options.toPath(), StandardCharsets.UTF_8);
            Map<String, String> m = new HashMap<String, String>();
            for (String line : lines) { int c = line.indexOf(':'); if (c > 0) m.put(line.substring(0, c), line.substring(c + 1).trim()); }
            for (int i = 0; i < OPTION.length; i++) {
                String v = m.get(OPTION[i]);
                if (v != null) apply(keys[i], v);
            }
        } catch (Exception ignored) { /* keep the defaults */ }
    }

    private static final Map<String, Integer> NAMED = new HashMap<String, Integer>();
    static {
        Object[][] t = { {"space", 32, "SPACE"}, {"left.shift", 340, "SHIFT"}, {"right.shift", 344, "SHIFT"}, {"left.control", 341, "CTRL"}, {"right.control", 345, "CTRL"},
            {"left.alt", 342, "ALT"}, {"right.alt", 346, "ALT"}, {"tab", 258, "TAB"}, {"up", 265, "UP"}, {"down", 264, "DOWN"}, {"left", 263, "LEFT"}, {"right", 262, "RIGHT"},
            {"enter", 257, "ENTER"}, {"caps.lock", 280, "CAPS"} };
        for (Object[] o : t) NAMED.put((String) o[0], (Integer) o[1]);
    }

    static void apply(Key k, String v) {
        if (v.startsWith("key.mouse.")) {
            String b = v.substring(10);
            k.mouse = true;
            if (b.equals("left")) k.code = 0; else if (b.equals("right")) k.code = 1; else if (b.equals("middle")) k.code = 2; else if (b.matches("\\d+")) k.code = Integer.parseInt(b) - 1;
            k.label = b.equals("left") ? "LMB" : b.equals("right") ? "RMB" : b.equals("middle") ? "MMB" : "M" + b;
        } else if (v.startsWith("key.keyboard.")) {
            String n = v.substring(13);
            k.mouse = false;
            if (n.length() == 1) { char ch = Character.toUpperCase(n.charAt(0)); k.code = ch; k.label = String.valueOf(ch); }
            else if (NAMED.containsKey(n)) { k.code = NAMED.get(n); k.label = n.equals("space") ? "SPACE" : n.contains("shift") ? "SHIFT" : n.contains("control") ? "CTRL" : n.contains("alt") ? "ALT" : n.toUpperCase();
                if (n.equals("caps.lock")) k.label = "CAPS"; }
        }
    }
}
