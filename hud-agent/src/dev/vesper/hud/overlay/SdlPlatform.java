package dev.vesper.hud.overlay;

import org.lwjgl.BufferUtils;
import org.lwjgl.sdl.SDLKeyboard;
import org.lwjgl.sdl.SDLMouse;
import org.lwjgl.sdl.SDLVideo;

import java.nio.ByteBuffer;
import java.nio.FloatBuffer;
import java.nio.IntBuffer;

/** SDL3 (Minecraft 26.x). Keys are looked up by SDL scancode, so GLFW key numbers are translated first. */
final class SdlPlatform implements Platform {
    private final IntBuffer ia = BufferUtils.createIntBuffer(1), ib = BufferUtils.createIntBuffer(1);
    private final FloatBuffer fx = BufferUtils.createFloatBuffer(1), fy = BufferUtils.createFloatBuffer(1);

    public void framebufferSize(long w, int[] a, int[] b) { SDLVideo.SDL_GetWindowSizeInPixels(w, ia, ib); a[0] = ia.get(0); b[0] = ib.get(0); }
    public void windowSize(long w, int[] a, int[] b) { SDLVideo.SDL_GetWindowSize(w, ia, ib); a[0] = ia.get(0); b[0] = ib.get(0); }

    public boolean key(long w, int glfwKey) {
        int sc = scancode(glfwKey);
        ByteBuffer ks = SDLKeyboard.SDL_GetKeyboardState();
        return sc > 0 && ks != null && sc < ks.capacity() && ks.get(sc) != 0;
    }

    public boolean mouse(long w, int b) {
        int mask = SDLMouse.SDL_GetMouseState(fx, fy);
        int bit = b == 0 ? 1 : b == 1 ? 4 : b == 2 ? 2 : 1 << b; // SDL: left 1, middle 2, right 4
        return (mask & bit) != 0;
    }

    public void cursor(long w, double[] x, double[] y) { SDLMouse.SDL_GetMouseState(fx, fy); x[0] = fx.get(0); y[0] = fy.get(0); }
    public boolean cursorFree(long w) { return !SDLMouse.SDL_GetWindowRelativeMouseMode(w); }

    /** GLFW key number to SDL scancode (0 when we do not know it). */
    static int scancode(int k) {
        if (k >= 65 && k <= 90) return 4 + (k - 65);          // A-Z
        if (k >= 49 && k <= 57) return 30 + (k - 49);         // 1-9
        if (k == 48) return 39;                               // 0
        if (k >= 290 && k <= 301) return 58 + (k - 290);      // F1-F12
        switch (k) {
            case 32: return 44;   // space
            case 340: return 225; case 344: return 229; // shifts
            case 341: return 224; case 345: return 228; // controls
            case 342: return 226; case 346: return 230; // alts
            case 258: return 43; case 257: return 40; case 280: return 57; // tab, enter, caps
            case 265: return 82; case 264: return 81; case 263: return 80; case 262: return 79; // arrows
            case 61: return 46; case 45: return 45; case 334: return 87; case 333: return 86;
            default: return 0;
        }
    }
}
