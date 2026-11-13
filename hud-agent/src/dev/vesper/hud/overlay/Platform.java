package dev.vesper.hud.overlay;

/**
 * What the overlay needs from the game's windowing library. Minecraft up to 1.21.x uses GLFW; Minecraft 26.x
 * opens its window with SDL3 instead. Key and button numbers are GLFW's (what options.txt maps to).
 */
interface Platform {
    void framebufferSize(long window, int[] w, int[] h);
    void windowSize(long window, int[] w, int[] h);
    boolean key(long window, int glfwKey);
    boolean mouse(long window, int glfwButton);
    void cursor(long window, double[] x, double[] y);
    /** The pointer is free (a menu is open) rather than captured for looking around. */
    boolean cursorFree(long window);
}
