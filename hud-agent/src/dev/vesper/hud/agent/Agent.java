package dev.vesper.hud.agent;

import java.io.File;
import java.lang.instrument.Instrumentation;
import java.net.URL;
import java.util.function.LongConsumer;

/**
 * Vesper's universal HUD agent, started with -javaagent. It does not depend on any mod loader or Minecraft
 * version: it hooks LWJGL's GLFW (present in every Minecraft since 1.13) and draws an overlay on each frame.
 */
public final class Agent {
    static final String GLFW_KEY = "vesper.hud.swap.glfw", SDL_KEY = "vesper.hud.swap.sdl";
    static volatile ClassLoader gameLoader;
    private static URL jar;

    private Agent() {}

    public static void premain(String args, Instrumentation inst) {
        try {
            jar = new File(Agent.class.getProtectionDomain().getCodeSource().getLocation().toURI()).toURI().toURL();
            System.getProperties().put(GLFW_KEY, new Bootstrap("glfw", GLFW_KEY));
            System.getProperties().put(SDL_KEY, new Bootstrap("sdl", SDL_KEY));
            inst.addTransformer(new GlfwPatch(), false);
            System.out.println("[Vesper HUD] agent loaded from " + jar);
            System.setProperty("vesper.hud.agent", "1"); // the Fabric add-on checks this to avoid drawing the same elements twice
        } catch (Throwable t) {
            System.err.println("[Vesper HUD] agent not started: " + t);
        }
    }

    /** The first frame builds the real overlay (inside the game's class loader) and swaps itself out. */
    private static final class Bootstrap implements LongConsumer {
        private final String platform, key;
        Bootstrap(String platform, String key) { this.platform = platform; this.key = key; }

        @Override
        public void accept(long window) {
            LongConsumer next = off;
            try {
                ClassLoader parent = gameLoader;
                if (parent != null) {
                    Class<?> c = new OverlayLoader(jar, parent).loadClass("dev.vesper.hud.overlay.Overlay");
                    next = (LongConsumer) c.getDeclaredConstructor(String.class).newInstance(platform);
                    System.out.println("[Vesper HUD] first frame reached the " + platform + " hook; overlay loaded through " + parent);
                }
            } catch (Throwable t) {
                System.err.println("[Vesper HUD] overlay not started: " + t);
                t.printStackTrace();
            }
            System.getProperties().put(key, next);
            next.accept(window);
        }
    }

    private static final LongConsumer off = new LongConsumer() { public void accept(long w) { } };
}
