import java.io.File;
import java.lang.reflect.Method;
import java.net.URL;
import java.net.URLClassLoader;

/**
 * Run with: java -javaagent:vesper-hud-agent.jar -cp <this> AgentMain <stubs dir>/
 * The fake GLFW lives in its OWN class loader (like Forge's), not the app loader, to prove the overlay is
 * loaded where it can see LWJGL, and that the call patched into glfwSwapBuffers reaches it.
 */
public class AgentMain {
    public static void main(String[] a) throws Exception {
        ClassLoader game = new URLClassLoader(new URL[] { new File(a[0]).toURI().toURL() }, ClassLoader.getPlatformClassLoader());
        Class<?> glfw = game.loadClass("org.lwjgl.glfw.GLFW");
        Method swap = glfw.getMethod("glfwSwapBuffers", long.class);
        for (int i = 0; i < 8; i++) { swap.invoke(null, 1L); Thread.sleep(15); }
        int swaps = glfw.getField("swaps").getInt(null);
        Class<?> gl11 = game.loadClass("org.lwjgl.opengl.GL11");
        int draws = gl11.getField("drawCalls").getInt(null), verts = gl11.getField("quadVertices").getInt(null);
        System.out.println("swaps=" + swaps + " drawCalls=" + draws + " vertices=" + verts + " agentProperty=" + System.getProperty("vesper.hud.agent"));
        boolean ok = swaps == 8 && draws >= 7 && verts > 0 && verts % 6 == 0 && "1".equals(System.getProperty("vesper.hud.agent"));
        System.out.println(ok ? "PASS" : "FAIL");
        System.exit(ok ? 0 : 1);
    }
}
