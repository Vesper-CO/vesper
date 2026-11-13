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
    private static final int MAX_QUADS = 3000, FLOATS = 8; // x y u v r g b a
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

    private static final String VS = "#version 150\nin vec2 pos; in vec2 uv; in vec4 col; out vec2 vUv; out vec4 vCol; uniform vec2 screen;\n"
        + "void main() { vUv = uv; vCol = col; gl_Position = vec4(pos.x / screen.x * 2.0 - 1.0, 1.0 - pos.y / screen.y * 2.0, 0.0, 1.0); }\n";
    private static final String FS = "#version 150\nin vec2 vUv; in vec4 vCol; out vec4 outColor; uniform sampler2D atlas;\n"
        + "void main() { outColor = vCol * texture(atlas, vUv); }\n";

    GlSurface(Font font) {
        hasSampler = GL.getCapabilities().OpenGL33;
        int oldPub = GL11.glGetInteger(0x88EF), oldTex, oldActive = GL11.glGetInteger(0x84E0);
        GL15.glBindBuffer(0x88EC, 0); // no pixel-unpack buffer while uploading from memory
        GL13.glActiveTexture(0x84C0);
        oldTex = GL11.glGetInteger(0x8069);
        tex = GL11.glGenTextures();
        GL11.glBindTexture(0x0DE1, tex);
        GL11.glTexParameteri(0x0DE1, 0x2801, 0x2601); // linear
        GL11.glTexParameteri(0x0DE1, 0x2800, 0x2601);
        GL11.glTexParameteri(0x0DE1, 0x2802, 0x812F); // clamp
        GL11.glTexParameteri(0x0DE1, 0x2803, 0x812F);
        ByteBuffer px = BufferUtils.createByteBuffer(font.texW * font.texH * 4);
        for (int i = 0; i < font.alpha.length; i++) { px.put((byte) 255).put((byte) 255).put((byte) 255).put(font.alpha[i]); }
        px.flip();
        int oldAlign = GL11.glGetInteger(0x0CF5);
        GL11.glPixelStorei(0x0CF5, 1);
        GL11.glTexImage2D(0x0DE1, 0, 0x1908, font.texW, font.texH, 0, 0x1908, 0x1401, px);
        GL11.glPixelStorei(0x0CF5, oldAlign);
        GL11.glBindTexture(0x0DE1, oldTex);
        GL13.glActiveTexture(oldActive);
        GL15.glBindBuffer(0x88EC, oldPub);

        int vs = shader(0x8B31, VS), fs = shader(0x8B30, FS);
        prog = GL20.glCreateProgram();
        GL20.glAttachShader(prog, vs); GL20.glAttachShader(prog, fs);
        GL20.glBindAttribLocation(prog, 0, "pos"); GL20.glBindAttribLocation(prog, 1, "uv"); GL20.glBindAttribLocation(prog, 2, "col");
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

    public void quad(float x0, float y0, float x1, float y1, float u0, float v0, float u1, float v1, int argb) {
        if (quads >= MAX_QUADS) return;
        float a = (argb >>> 24) / 255f, r = ((argb >> 16) & 255) / 255f, g = ((argb >> 8) & 255) / 255f, b = (argb & 255) / 255f;
        put(x0, y0, u0, v0, r, g, b, a); put(x1, y0, u1, v0, r, g, b, a); put(x1, y1, u1, v1, r, g, b, a);
        put(x0, y0, u0, v0, r, g, b, a); put(x1, y1, u1, v1, r, g, b, a); put(x0, y1, u0, v1, r, g, b, a);
        quads++;
    }

    private void put(float x, float y, float u, float v, float r, float g, float b, float a) {
        buf.put(x).put(y).put(u).put(v).put(r).put(g).put(b).put(a);
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
