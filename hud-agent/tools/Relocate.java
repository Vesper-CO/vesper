import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.commons.ClassRemapper;
import org.objectweb.asm.commons.Remapper;

import java.io.*;
import java.nio.file.*;
import java.util.jar.*;
import java.util.zip.*;

/**
 * Builds the agent jar: our classes plus ASM, with ASM moved to dev/vesper/hud/shaded/asm so it can never clash
 * with the ASM that Fabric, Forge or NeoForge load in the same game.
 * usage: Relocate <classesDir> <asm.jar> <extra resources dir> <out.jar>
 */
public class Relocate {
    static final String FROM = "org/objectweb/asm/", TO = "dev/vesper/hud/shaded/asm/";

    public static void main(String[] a) throws Exception {
        Remapper map = new Remapper() { public String map(String n) { return n.startsWith(FROM) ? TO + n.substring(FROM.length()) : n; } };
        Manifest mf = new Manifest();
        mf.getMainAttributes().put(Attributes.Name.MANIFEST_VERSION, "1.0");
        mf.getMainAttributes().putValue("Premain-Class", "dev.vesper.hud.agent.Agent");
        mf.getMainAttributes().putValue("Can-Redefine-Classes", "false");
        mf.getMainAttributes().putValue("Can-Retransform-Classes", "false");
        try (JarOutputStream out = new JarOutputStream(new FileOutputStream(a[3]), mf)) {
            Path root = Paths.get(a[0]);
            try (var walk = Files.walk(root)) {
                for (Path p : (Iterable<Path>) walk.filter(Files::isRegularFile)::iterator) {
                    String rel = root.relativize(p).toString().replace('\\', '/');
                    byte[] data = Files.readAllBytes(p);
                    if (rel.endsWith(".class")) data = remap(data, map);
                    put(out, rel, data);
                }
            }
            try (JarFile asm = new JarFile(a[1])) {
                for (var en = asm.entries(); en.hasMoreElements();) {
                    JarEntry e = en.nextElement();
                    if (!e.getName().startsWith(FROM) || !e.getName().endsWith(".class")) continue;
                    byte[] data = remap(asm.getInputStream(e).readAllBytes(), map);
                    put(out, TO + e.getName().substring(FROM.length()), data);
                }
            }
            Path extra = Paths.get(a[2]);
            if (Files.isDirectory(extra)) try (var walk = Files.walk(extra)) {
                for (Path p : (Iterable<Path>) walk.filter(Files::isRegularFile)::iterator) put(out, extra.relativize(p).toString().replace('\\', '/'), Files.readAllBytes(p));
            }
        }
    }

    static byte[] remap(byte[] in, Remapper m) {
        ClassReader cr = new ClassReader(in);
        ClassWriter cw = new ClassWriter(0);
        cr.accept(new ClassRemapper(cw, m), 0);
        return cw.toByteArray();
    }

    static void put(JarOutputStream out, String name, byte[] data) throws IOException {
        ZipEntry z = new ZipEntry(name);
        z.setTime(0); // reproducible: same sources, same jar
        out.putNextEntry(z);
        out.write(data);
        out.closeEntry();
    }
}
