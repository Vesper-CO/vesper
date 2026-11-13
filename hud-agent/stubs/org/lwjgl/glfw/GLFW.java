package org.lwjgl.glfw;
public final class GLFW {
    public static volatile int swaps, cursorMode = 0x34001, mouseDown, keyRShift;
    public static volatile double cursorX, cursorY;
    public static volatile boolean keyW = true;
    public static void glfwSwapBuffers(long window) { swaps++; }
    public static int glfwGetKey(long window, int key) { return key == 87 && keyW ? 1 : key == 344 ? keyRShift : 0; }
    public static int glfwGetMouseButton(long window, int button) { return button == 0 ? mouseDown : 0; }
    public static int glfwGetInputMode(long window, int mode) { return cursorMode; }
    public static void glfwGetCursorPos(long window, double[] x, double[] y) { x[0] = cursorX; y[0] = cursorY; }
    public static void glfwGetWindowSize(long window, int[] w, int[] h) { w[0] = 1280; h[0] = 720; }
    public static void glfwGetFramebufferSize(long window, int[] w, int[] h) { w[0] = 1280; h[0] = 720; }
}
