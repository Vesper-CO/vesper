package dev.vesper.hud.overlay;

import org.lwjgl.glfw.GLFW;

final class GlfwPlatform implements Platform {
    public void framebufferSize(long w, int[] a, int[] b) { GLFW.glfwGetFramebufferSize(w, a, b); }
    public void windowSize(long w, int[] a, int[] b) { GLFW.glfwGetWindowSize(w, a, b); }
    public boolean key(long w, int k) { return GLFW.glfwGetKey(w, k) == 1; }
    public boolean mouse(long w, int b) { return GLFW.glfwGetMouseButton(w, b) == 1; }
    public void cursor(long w, double[] x, double[] y) { GLFW.glfwGetCursorPos(w, x, y); }
    public boolean cursorFree(long w) { return GLFW.glfwGetInputMode(w, 0x33001) == 0x34001; }
}
