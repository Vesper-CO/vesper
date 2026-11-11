package dev.vesper.hud.agent;

import java.net.URL;
import java.net.URLClassLoader;

/**
 * Loads the overlay classes in a child of the game's own class loader, so they can see LWJGL no matter how
 * the mod loader arranges its class loaders. Overlay classes are always taken from our jar first.
 */
final class OverlayLoader extends URLClassLoader {
    OverlayLoader(URL jar, ClassLoader gameLoader) { super(new URL[] { jar }, gameLoader); }

    @Override
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        if (!name.startsWith("dev.vesper.hud.overlay.")) return super.loadClass(name, resolve);
        synchronized (getClassLoadingLock(name)) {
            Class<?> c = findLoadedClass(name);
            if (c == null) c = findClass(name);
            if (resolve) resolveClass(c);
            return c;
        }
    }
}
