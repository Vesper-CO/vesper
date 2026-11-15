package dev.vesper.hud.overlay;

import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL14;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL30;
import org.lwjgl.opengl.GL33;

import java.nio.ByteBuffer;
import java.nio.FloatBuffer;

/**
 * Draws the quads on top of the finished frame with OpenGL 3.2+ (core or compatibility). It owns its own
 * program, buffers and texture, and puts back every piece of GL state it touches, so the game never notices.
 */
final class GlSurface implements Hud.Sink {
    private static final int MAX_QUADS = 3000, FLOATS = 15; // x y u v r g b a | shape: halfW halfH radius width | local x y, mode
    private int prog, vao, vbo, tex, uScreen;
    private final FloatBuffer buf = BufferUtils.createFloatBuffer(MAX_QUADS * 6 * FLOATS);
    private int quads;

    // state saved in begin() and restored in end()
    private int sProg, sVao, sArray, sActive, sTex, sFbo, sBlendSrcRgb, sBlendDstRgb, sBlendSrcA, sBlendDstA;
    private final int[] sViewport = new int[4];
    private boolean sBlend, sDepth, sCull, sScissor, sStencil, sDepthMask, sSrgb, sLogic, sDiscard;
    private int sSampler;
    private boolean hasSampler;
    private final ByteBuffer sMask = BufferUtils.createByteBuffer(4);
    int lastQuads() { return quads; }

    private static final String VS = "#version 150\nin vec2 pos; in vec2 uv; in vec4 col; in vec4 sh; in vec3 loc; out vec2 vUv; out vec4 vCol; out vec4 vSh; out vec3 vLoc; uniform vec2 screen;\n"
        + "void main() { vUv = uv; vCol = col; vSh = sh; vLoc = loc; gl_Position = vec4(pos.x / screen.x * 2.0 - 1.0, 1.0 - pos.y / screen.y * 2.0, 0.0, 1.0); }\n";
    /**
     * Text and plain rectangles sample the atlas. Shapes (rounded boxes, outlines, halos, the logo's spark) are worked
     * out per pixel from a distance to the shape's edge, which gives smooth anti-aliased edges at any size:
     * mode 1 fill, 2 outline `w` thick, 3 halo fading over `w`, 4 spark, 5 the logo's crescent moon.
     */
    private static final String FS = "#version 150\nin vec2 vUv; in vec4 vCol; in vec4 vSh; in vec3 vLoc; out vec4 outColor; uniform sampler2D atlas;\n"
        + "float cover() {\n"
        + "  if (vLoc.z < 0.5) return 1.0;\n"
        + "  vec2 p = vLoc.xy;\n"
        + "  if (vLoc.z > 4.5) { float r = vSh.x; float da = length(p) - r; float db = length(p - vec2(0.7119, -0.4963) * r) - 0.8225 * r; return clamp(0.5 - da, 0.0, 1.0) * clamp(db + 0.5, 0.0, 1.0); }\n"
        + "  if (vLoc.z > 3.5) { vec2 q = abs(p) / vSh.xy; float f = pow(q.x, 0.65) + pow(q.y, 0.65) - 1.0; return clamp(0.5 - f / max(fwidth(f), 0.0001), 0.0, 1.0); }\n"
        + "  vec2 q = abs(p) - vSh.xy + vec2(vSh.z);\n"
        + "  float d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - vSh.z;\n"
        + "  if (vLoc.z < 1.5) return clamp(0.5 - d, 0.0, 1.0);\n"
        + "  if (vLoc.z < 2.5) return clamp(0.5 - d, 0.0, 1.0) * clamp(d + vSh.w + 0.5, 0.0, 1.0);\n"
        + "  float t = 1.0 - clamp(d / max(vSh.w, 0.001), 0.0, 1.0); return t * t;\n"
        + "}\n"
        + "void main() {\n"
        + "  vec2 dx = dFdx(vUv) * 0.25, dy = dFdy(vUv) * 0.25;\n"
        + "  vec4 t = vec4(1.0);\n"
        + "  if (vLoc.z < 0.5) t = 0.25 * (textureLod(atlas, vUv + dx + dy, 0.0) + textureLod(atlas, vUv + dx - dy, 0.0) + textureLod(atlas, vUv - dx + dy, 0.0) + textureLod(atlas, vUv - dx - dy, 0.0));\n"
        + "  vec4 c = vCol * t; c.a *= cover(); outColor = c;\n"
        + "}\n\n";

    GlSurface(Font font) {
        whiteU = ((95 % font.cols) * font.cellW + font.cellW / 2f) / font.texW;
        whiteV = ((95 / font.cols) * font.cellH + font.cellH / 2f) / font.texH;
        hasSampler = GL.getCapabilities().OpenGL33;
        int oldPub = GL11.glGetInteger(0x88EF), oldTex, oldActive = GL11.glGetInteger(0x84E0);
        GL15.glBindBuffer(0x88EC, 0); // no pixel-unpack buffer while uploading from memory
        GL13.glActiveTexture(0x84C0);
        oldTex = GL11.glGetInteger(0x8069);
        tex = GL11.glGenTextures();
        GL11.glBindTexture(0x0DE1, tex);
        GL11.glTexParameteri(0x0DE1, 0x2800, 0x2601); // magnify: linear
        GL11.glTexParameteri(0x0DE1, 0x2802, 0x812F); // clamp
        GL11.glTexParameteri(0x0DE1, 0x2803, 0x812F);
        ByteBuffer px = BufferUtils.createByteBuffer(font.texW * font.texH * 4);
        for (int i = 0; i < font.alpha.length; i++) { px.put((byte) 255).put((byte) 255).put((byte) 255).put(font.alpha[i]); }
        px.flip();
        // The game leaves its own texture-upload settings behind (row length, skips, alignment). Take them out of the picture:
        // reset them for this upload, and send the atlas one row at a time, so a stray row length cannot shear it.
        int[] store = { 0x0CF5, 0x0CF2, 0x0CF3, 0x0CF4, 0x806E, 0x806D }; // alignment, row length, skip rows, skip pixels, image height, skip images
        int[] old = new int[store.length];
        for (int i = 0; i < store.length; i++) { old[i] = GL11.glGetInteger(store[i]); GL11.glPixelStorei(store[i], i == 0 ? 1 : 0); }
        GL11.glTexImage2D(0x0DE1, 0, 0x1908, font.texW, font.texH, 0, 0x1908, 0x1401, (ByteBuffer) null);
        for (int row = 0; row < font.texH; row++) {
            ByteBuffer line = px.duplicate();
            line.limit((row + 1) * font.texW * 4);
            line.position(row * font.texW * 4);
            GL11.glTexSubImage2D(0x0DE1, 0, 0, row, font.texW, 1, 0x1908, 0x1401, line);
        }
        GL11.glTexParameteri(0x0DE1, 0x2801, 0x2601); // minify: linear (the shader averages a few taps, see FS)
        for (int i = 0; i < store.length; i++) GL11.glPixelStorei(store[i], old[i]);
        GL11.glBindTexture(0x0DE1, oldTex);
        GL13.glActiveTexture(oldActive);
        GL15.glBindBuffer(0x88EC, oldPub);

        int vs = shader(0x8B31, VS), fs = shader(0x8B30, FS);
        prog = GL20.glCreateProgram();
        GL20.glAttachShader(prog, vs); GL20.glAttachShader(prog, fs);
        GL20.glBindAttribLocation(prog, 0, "pos"); GL20.glBindAttribLocation(prog, 1, "uv"); GL20.glBindAttribLocation(prog, 2, "col");
        GL20.glBindAttribLocation(prog, 3, "sh"); GL20.glBindAttribLocation(prog, 4, "loc");
        GL20.glLinkProgram(prog);
        if (GL20.glGetProgrami(prog, 0x8B82) == 0) throw new IllegalStateException("link failed: " + GL20.glGetProgramInfoLog(prog, 1024));
        GL20.glDeleteShader(vs); GL20.glDeleteShader(fs);
        uScreen = GL20.glGetUniformLocation(prog, "screen");
        int uAtlas = GL20.glGetUniformLocation(prog, "atlas");

        int oldVao = GL11.glGetInteger(0x85B5), oldArray = GL11.glGetInteger(0x8894), oldProg = GL11.glGetInteger(0x8B8D);
        vao = GL30.glGenVertexArrays();
        vbo = GL15.glGenBuffers();
        GL30.glBindVertexArray(vao);
        GL15.glBindBuffer(0x8892, vbo);
        GL20.glEnableVertexAttribArray(0); GL20.glVertexAttribPointer(0, 2, 0x1406, false, FLOATS * 4, 0L);
        GL20.glEnableVertexAttribArray(1); GL20.glVertexAttribPointer(1, 2, 0x1406, false, FLOATS * 4, 8L);
        GL20.glEnableVertexAttribArray(2); GL20.glVertexAttribPointer(2, 4, 0x1406, false, FLOATS * 4, 16L);
        GL20.glEnableVertexAttribArray(3); GL20.glVertexAttribPointer(3, 4, 0x1406, false, FLOATS * 4, 32L);
        GL20.glEnableVertexAttribArray(4); GL20.glVertexAttribPointer(4, 3, 0x1406, false, FLOATS * 4, 48L);
        GL20.glUseProgram(prog);
        GL20.glUniform1i(uAtlas, 0);
        GL20.glUseProgram(oldProg);
        GL30.glBindVertexArray(oldVao);
        GL15.glBindBuffer(0x8892, oldArray);
        drain();
    }

    private static int shader(int type, String src) {
        int s = GL20.glCreateShader(type);
        GL20.glShaderSource(s, src);
        GL20.glCompileShader(s);
        if (GL20.glGetShaderi(s, 0x8B81) == 0) throw new IllegalStateException("shader failed: " + GL20.glGetShaderInfoLog(s, 1024));
        return s;
    }

    private static void drain() { for (int i = 0; i < 16 && GL11.glGetError() != 0; i++) { /* clear any error we caused */ } }

    void begin(int w, int h) {
        drain();
        quads = 0; buf.clear();
        sProg = GL11.glGetInteger(0x8B8D); sVao = GL11.glGetInteger(0x85B5); sArray = GL11.glGetInteger(0x8894);
        sActive = GL11.glGetInteger(0x84E0); sFbo = GL11.glGetInteger(0x8CA6);
        GL11.glGetIntegerv(0x0BA2, sViewport);
        sBlend = GL11.glIsEnabled(0x0BE2); sDepth = GL11.glIsEnabled(0x0B71); sCull = GL11.glIsEnabled(0x0B44);
        sScissor = GL11.glIsEnabled(0x0C11); sStencil = GL11.glIsEnabled(0x0B90); sDepthMask = GL11.glGetBoolean(0x0B72);
        sBlendSrcRgb = GL11.glGetInteger(0x80C9); sBlendDstRgb = GL11.glGetInteger(0x80C8); sBlendSrcA = GL11.glGetInteger(0x80CB); sBlendDstA = GL11.glGetInteger(0x80CA);
        GL13.glActiveTexture(0x84C0);
        sTex = GL11.glGetInteger(0x8069);
        // Newer game versions may leave a sampler object, a colour write mask, sRGB or a logic op set: neutralise them
        sSampler = hasSampler ? GL11.glGetInteger(0x8919) : 0;
        if (hasSampler) GL33.glBindSampler(0, 0);
        sMask.clear(); GL11.glGetBooleanv(0x0C23, sMask);
        sSrgb = GL11.glIsEnabled(0x8DB9); sLogic = GL11.glIsEnabled(0x0BF2); sDiscard = GL11.glIsEnabled(0x8C89);
        GL11.glColorMask(true, true, true, true);
        GL11.glDisable(0x8DB9); GL11.glDisable(0x0BF2); GL11.glDisable(0x8C89);

        GL30.glBindFramebuffer(0x8D40, 0);
        GL11.glViewport(0, 0, w, h);
        GL11.glDisable(0x0B71); GL11.glDisable(0x0B44); GL11.glDisable(0x0C11); GL11.glDisable(0x0B90);
        GL11.glDepthMask(false);
        GL11.glEnable(0x0BE2);
        GL14.glBlendFuncSeparate(0x0302, 0x0303, 1, 0x0303);
        GL20.glUseProgram(prog);
        GL20.glUniform2f(uScreen, w, h);
        GL11.glBindTexture(0x0DE1, tex);
        GL30.glBindVertexArray(vao);
    }

    public boolean shapes() { return true; }

    public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) {
        if (quads >= MAX_QUADS) return;
        float a = (argb >>> 24) / 255f, r = ((argb >> 16) & 255) / 255f, g = ((argb >> 8) & 255) / 255f, b = (argb & 255) / 255f;
        put(x0, y0, u0, v0, r, g, b, a, 0, 0, 0, 0, 0, 0, 0); put(x1, y0, u1, v0, r, g, b, a, 0, 0, 0, 0, 0, 0, 0); put(x1, y1, u1, v1, r, g, b, a, 0, 0, 0, 0, 0, 0, 0);
        put(x0, y0, u0, v0, r, g, b, a, 0, 0, 0, 0, 0, 0, 0); put(x1, y1, u1, v1, r, g, b, a, 0, 0, 0, 0, 0, 0, 0); put(x0, y1, u0, v1, r, g, b, a, 0, 0, 0, 0, 0, 0, 0);
        quads++;
    }

    /** A shape in the quad (qx0..qy1) whose box is (bx, by, bw, bh); `argb` is the top-left colour and `argb2` the bottom-right (a gradient). */
    public void shape(float qx0, float qy0, float qx1, float qy1, float bx, float by, float bw, float bh, float r, float w, int mode, int argb, int argb2) {
        if (quads >= MAX_QUADS) return;
        float[] c1 = rgba(argb), c2 = rgba(argb2), cm = { (c1[0] + c2[0]) / 2, (c1[1] + c2[1]) / 2, (c1[2] + c2[2]) / 2, (c1[3] + c2[3]) / 2 };
        float hw = bw / 2f, hh = bh / 2f, cx = bx + hw, cy = by + hh, m = mode;
        float u = whiteU, v = whiteV;
        put(qx0, qy0, u, v, c1[0], c1[1], c1[2], c1[3], hw, hh, r, w, qx0 - cx, qy0 - cy, m);
        put(qx1, qy0, u, v, cm[0], cm[1], cm[2], cm[3], hw, hh, r, w, qx1 - cx, qy0 - cy, m);
        put(qx1, qy1, u, v, c2[0], c2[1], c2[2], c2[3], hw, hh, r, w, qx1 - cx, qy1 - cy, m);
        put(qx0, qy0, u, v, c1[0], c1[1], c1[2], c1[3], hw, hh, r, w, qx0 - cx, qy0 - cy, m);
        put(qx1, qy1, u, v, c2[0], c2[1], c2[2], c2[3], hw, hh, r, w, qx1 - cx, qy1 - cy, m);
        put(qx0, qy1, u, v, cm[0], cm[1], cm[2], cm[3], hw, hh, r, w, qx0 - cx, qy1 - cy, m);
        quads++;
    }

    private float whiteU, whiteV;

    private static float[] rgba(int argb) { return new float[] { ((argb >> 16) & 255) / 255f, ((argb >> 8) & 255) / 255f, (argb & 255) / 255f, (argb >>> 24) / 255f }; }

    private void put(float x, float y, float u, float v, float r, float g, float b, float a, float hw, float hh, float rad, float w, float lx, float ly, float mode) {
        buf.put(x).put(y).put(u).put(v).put(r).put(g).put(b).put(a).put(hw).put(hh).put(rad).put(w).put(lx).put(ly).put(mode);
    }

    void end() {
        if (quads > 0) {
            buf.flip();
            GL15.glBindBuffer(0x8892, vbo);
            GL15.glBufferData(0x8892, buf, 0x88E0);
            GL11.glDrawArrays(4, 0, quads * 6);
        }
        GL11.glBindTexture(0x0DE1, sTex);
        if (hasSampler) GL33.glBindSampler(0, sSampler);
        GL11.glColorMask(sMask.get(0) != 0, sMask.get(1) != 0, sMask.get(2) != 0, sMask.get(3) != 0);
        if (sSrgb) GL11.glEnable(0x8DB9);
        if (sLogic) GL11.glEnable(0x0BF2);
        if (sDiscard) GL11.glEnable(0x8C89);
        GL13.glActiveTexture(sActive);
        GL30.glBindVertexArray(sVao);
        GL15.glBindBuffer(0x8892, sArray);
        GL20.glUseProgram(sProg);
        GL14.glBlendFuncSeparate(sBlendSrcRgb, sBlendDstRgb, sBlendSrcA, sBlendDstA);
        if (!sBlend) GL11.glDisable(0x0BE2);
        if (sDepth) GL11.glEnable(0x0B71);
        if (sCull) GL11.glEnable(0x0B44);
        if (sScissor) GL11.glEnable(0x0C11);
        if (sStencil) GL11.glEnable(0x0B90);
        GL11.glDepthMask(sDepthMask);
        GL11.glViewport(sViewport[0], sViewport[1], sViewport[2], sViewport[3]);
        GL30.glBindFramebuffer(0x8D40, sFbo);
        drain();
    }
}
