package org.lwjgl.opengl;
public final class GL20 {
    public static int glCreateShader(int t) { return 1; }
    public static void glShaderSource(int s, CharSequence src) { }
    public static void glCompileShader(int s) { }
    public static int glGetShaderi(int s, int p) { return 1; }
    public static String glGetShaderInfoLog(int s, int n) { return ""; }
    public static int glCreateProgram() { return 1; }
    public static void glAttachShader(int p, int s) { }
    public static void glBindAttribLocation(int p, int i, CharSequence n) { }
    public static void glLinkProgram(int p) { }
    public static int glGetProgrami(int p, int n) { return 1; }
    public static String glGetProgramInfoLog(int p, int n) { return ""; }
    public static void glDeleteShader(int s) { }
    public static int glGetUniformLocation(int p, CharSequence n) { return 0; }
    public static void glUseProgram(int p) { }
    public static void glUniform1i(int l, int v) { }
    public static void glUniform2f(int l, float a, float b) { }
    public static void glEnableVertexAttribArray(int i) { }
    public static void glVertexAttribPointer(int i, int size, int type, boolean norm, int stride, long ptr) { }
}
