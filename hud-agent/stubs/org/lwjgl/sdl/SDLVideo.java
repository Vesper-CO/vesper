package org.lwjgl.sdl;
import java.nio.IntBuffer;
public final class SDLVideo {
    public static volatile int swaps;
    public static boolean SDL_GL_SwapWindow(long window) { swaps++; return true; }
    public static boolean SDL_GetWindowSizeInPixels(long w, IntBuffer a, IntBuffer b) { a.put(0, 1280); b.put(0, 720); return true; }
    public static boolean SDL_GetWindowSize(long w, IntBuffer a, IntBuffer b) { a.put(0, 1280); b.put(0, 720); return true; }
}
