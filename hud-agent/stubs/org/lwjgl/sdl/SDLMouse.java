package org.lwjgl.sdl;
import java.nio.FloatBuffer;
public final class SDLMouse {
    public static volatile int mask;
    public static volatile float x, y;
    public static volatile boolean relative;
    public static int SDL_GetMouseState(FloatBuffer a, FloatBuffer b) { a.put(0, x); b.put(0, y); return mask; }
    public static boolean SDL_GetWindowRelativeMouseMode(long window) { return relative; }
}
