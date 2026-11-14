import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;

/**
 * The pause-menu button: playing (pointer captured) -> Esc opens the pause menu (pointer freed) -> the Vesper button is
 * drawn and a click on it opens the editor -> a drag saves -> Done leaves. Also: a menu opened WITHOUT Esc (inventory)
 * must not show the button, so a click in the same place does nothing. Window is the fake 1280x720 (GUI scale 3).
 */
public class PauseTest {
    static Class<?> glfw, kbd, mouse, gl11;
    static Method swap;
    static boolean sdl;

    static void set(String f, Object v) throws Exception {
        if (!sdl) { Field x = glfw.getField(f); if (v instanceof Integer) x.setInt(null, (Integer) v); else x.setDouble(null, ((Number) v).doubleValue()); return; }
        if (f.equals("keyEsc")) ((java.nio.ByteBuffer) kbd.getField("state").get(null)).put(41, (byte) ((Integer) v).intValue());
        else if (f.equals("cursorMode")) mouse.getField("relative").setBoolean(null, ((Integer) v) == 0x34003);
        else if (f.equals("cursorX")) mouse.getField("x").setFloat(null, ((Number) v).floatValue());
        else if (f.equals("cursorY")) mouse.getField("y").setFloat(null, ((Number) v).floatValue());
        else if (f.equals("mouseDown")) mouse.getField("mask").setInt(null, (Integer) v);
    }
    static void frames(int n) throws Exception { for (int i = 0; i < n; i++) { swap.invoke(null, 1L); Thread.sleep(5); } }
    static int draws() throws Exception { return gl11.getField("drawCalls").getInt(null); }
    static void click(double x, double y) throws Exception { set("cursorX", x); set("cursorY", y); frames(2); set("mouseDown", 1); frames(2); set("mouseDown", 0); frames(2); }

    public static void main(String[] a) throws Exception {
        ClassLoader game = new URLClassLoader(new URL[] { new File(a[0]).toURI().toURL() }, ClassLoader.getPlatformClassLoader());
        sdl = a.length > 1 && a[1].equals("sdl");
        glfw = game.loadClass(sdl ? "org.lwjgl.sdl.SDLVideo" : "org.lwjgl.glfw.GLFW");
        kbd = sdl ? game.loadClass("org.lwjgl.sdl.SDLKeyboard") : null;
        mouse = sdl ? game.loadClass("org.lwjgl.sdl.SDLMouse") : null;
        gl11 = game.loadClass("org.lwjgl.opengl.GL11");
        swap = glfw.getMethod(sdl ? "SDL_GL_SwapWindow" : "glfwSwapBuffers", long.class);
        File cfg = new File(System.getProperty("vesper.hud.config"));
        // where the button is at 1280x720, GUI scale 3 (26.x: end of the icon strip; older: beside Back to Game)
        double bx = sdl ? 789 + 30 : 957 + 30, by = sdl ? 348 + 30 : 204 + 30;
        frames(3);

        // 1. a menu opened without Esc (like the inventory): the button is not there, a click on its spot does nothing
        set("cursorMode", 0x34003); frames(2);
        set("cursorMode", 0x34001); frames(3);
        click(bx, by);
        click(640, 360); // a drag target to detect edit mode below
        boolean noButtonWithoutEsc = !cfg.exists();

        // 2. Esc opens the pause menu
        set("cursorMode", 0x34003); frames(2);
        set("keyEsc", 1); set("cursorMode", 0x34001); frames(2); set("keyEsc", 0); frames(3);
        int before = draws();
        frames(3);
        boolean buttonDrawn = draws() - before >= 3;
        // hovering the button draws more (tooltip and glow) than not hovering
        set("cursorX", 5.0); set("cursorY", 700.0); frames(3); int a0 = draws(); frames(1); int plain = draws() - a0;
        click(bx, by); // opens the editor
        // 3. drag the FPS element (top-left) to the middle in edit mode
        set("cursorX", 30.0); set("cursorY", 30.0); set("mouseDown", 1); frames(2);
        set("cursorX", 330.0); set("cursorY", 200.0); frames(2);
        set("cursorX", 640.0); set("cursorY", 360.0); frames(2);
        set("mouseDown", 0); frames(3);
        boolean saved = cfg.isFile();
        String json = saved ? new String(Files.readAllBytes(cfg.toPath()), "UTF-8") : "";
        boolean moved = EditTest.dev(json) != null;
        // 4. leaving the menu (back to the game) ends edit mode and the game keeps drawing
        set("cursorMode", 0x34003); frames(3);
        int d = draws(); frames(3);
        boolean ok = noButtonWithoutEsc && buttonDrawn && saved && moved && draws() > d;
        System.out.println((sdl ? "[sdl] " : "[glfw] ") + "noButtonWithoutEsc=" + noButtonWithoutEsc + " buttonDrawn=" + buttonDrawn + " editOpenedByClick+drag saved=" + saved + " moved=" + moved);
        System.out.println(ok ? "PASS" : "FAIL");
        System.exit(ok ? 0 : 1);
    }
}
