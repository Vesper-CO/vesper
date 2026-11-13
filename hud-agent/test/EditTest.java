import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;

/** Drives the in-game editor through the fake GLFW: open edit mode, drag the FPS element, close, check the file. */
public class EditTest {
    static Class<?> glfw;
    static Method swap;

    static void set(String f, Object v) throws Exception { Field x = glfw.getField(f); if (v instanceof Integer) x.setInt(null, (Integer) v); else x.setDouble(null, ((Number) v).doubleValue()); }
    static void frames(int n) throws Exception { for (int i = 0; i < n; i++) { swap.invoke(null, 1L); Thread.sleep(5); } }

    public static void main(String[] a) throws Exception {
        ClassLoader game = new URLClassLoader(new URL[] { new File(a[0]).toURI().toURL() }, ClassLoader.getPlatformClassLoader());
        glfw = game.loadClass("org.lwjgl.glfw.GLFW");
        swap = glfw.getMethod("glfwSwapBuffers", long.class);
        File cfg = new File(System.getProperty("vesper.hud.config"));
        frames(3);
        // a captured pointer (playing, not in a menu): Right Shift must NOT start editing
        set("cursorMode", 0x34003); set("keyRShift", 1); frames(2); set("keyRShift", 0); frames(2);
        boolean noEditWhilePlaying = !cfg.exists();
        // open a menu, press Right Shift: edit mode
        set("cursorMode", 0x34001); set("keyRShift", 1); frames(2); set("keyRShift", 0); frames(2);
        // press on the FPS element, drag to the middle, release
        set("cursorX", 30.0); set("cursorY", 30.0); set("mouseDown", 1); frames(2);
        set("cursorX", 330.0); set("cursorY", 200.0); frames(2);
        set("cursorX", 640.0); set("cursorY", 360.0); frames(2);
        set("mouseDown", 0); frames(2);
        boolean saved = cfg.isFile();
        String json = saved ? new String(Files.readAllBytes(cfg.toPath()), "UTF-8") : "";
        System.out.println(json);
        // Right Shift again leaves edit mode; the game keeps drawing
        set("keyRShift", 1); frames(2); set("keyRShift", 0); frames(3);
        int draws = game.loadClass("org.lwjgl.opengl.GL11").getField("drawCalls").getInt(null);
        Object x = null;
        try { x = dev(json); } catch (Exception e) { System.out.println("bad json: " + e); }
        boolean ok = noEditWhilePlaying && saved && x != null && draws >= 10;
        System.out.println("noEditWhilePlaying=" + noEditWhilePlaying + " saved=" + saved + " fpsPos=" + x + " drawCalls=" + draws);
        System.out.println(ok ? "PASS" : "FAIL");
        System.exit(ok ? 0 : 1);
    }

    /** Pull elements.fps.x and .y out of the saved file; they must have moved to about the middle of the screen. */
    static Object dev(String json) {
        java.util.regex.Matcher m = java.util.regex.Pattern.compile("\"fps\": \\{[^}]*\"x\": ([0-9.]+),\\s*\"y\": ([0-9.]+)").matcher(json);
        if (!m.find()) return null;
        double fx = Double.parseDouble(m.group(1)), fy = Double.parseDouble(m.group(2));
        return fx > 0.45 && fx < 0.62 && fy > 0.4 && fy < 0.6 ? fx + "," + fy : null;
    }
}
