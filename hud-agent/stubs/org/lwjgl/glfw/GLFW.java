package org.lwjgl.glfw;
public final class GLFW {
    public static volatile int swaps;
    public static volatile boolean keyW = true;
    public static void glfwSwapBuffers(long window) { swaps++; }
    public static int glfwGetKey(long window, int key) { return key == 87 && keyW ? 1 : 0; }
    public static int glfwGetMouseButton(long window, int button) { return 0; }
    public static void glfwGetFramebufferSize(long window, int[] w, int[] h) { w[0] = 1280; h[0] = 720; }
}
