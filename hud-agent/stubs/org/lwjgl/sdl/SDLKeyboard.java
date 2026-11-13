package org.lwjgl.sdl;
import java.nio.ByteBuffer;
public final class SDLKeyboard {
    public static final ByteBuffer state = ByteBuffer.allocate(512);
    public static ByteBuffer SDL_GetKeyboardState() { return state; }
}
