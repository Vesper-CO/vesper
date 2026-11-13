package org.lwjgl;
import java.nio.*;
/** Compile-time stand-in for LWJGL (never shipped): the game provides the real one at runtime. */
public final class BufferUtils {
    public static ByteBuffer createByteBuffer(int n) { return ByteBuffer.allocateDirect(n).order(ByteOrder.nativeOrder()); }
    public static IntBuffer createIntBuffer(int n) { return createByteBuffer(n * 4).asIntBuffer(); }
    public static FloatBuffer createFloatBuffer(int n) { return createByteBuffer(n * 4).asFloatBuffer(); }
}
