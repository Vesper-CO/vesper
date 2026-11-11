package org.lwjgl.opengl;
import java.nio.ByteBuffer;
public final class GL11 {
    public static volatile int drawCalls, quadVertices, textures;
    public static int glGetInteger(int p) { return 0; }
    public static void glGetIntegerv(int p, int[] v) { }
    public static boolean glIsEnabled(int p) { return false; }
    public static boolean glGetBoolean(int p) { return true; }
    public static int glGenTextures() { textures++; return textures; }
    public static void glBindTexture(int t, int id) { }
    public static void glTexParameteri(int t, int p, int v) { }
    public static void glPixelStorei(int p, int v) { }
    public static void glTexImage2D(int t, int l, int f, int w, int h, int b, int fmt, int type, ByteBuffer px) { }
    public static void glViewport(int x, int y, int w, int h) { }
    public static void glEnable(int c) { }
    public static void glDisable(int c) { }
    public static void glDepthMask(boolean b) { }
    public static void glDrawArrays(int mode, int first, int count) { drawCalls++; quadVertices += count; }
    public static int glGetError() { return 0; }
}
