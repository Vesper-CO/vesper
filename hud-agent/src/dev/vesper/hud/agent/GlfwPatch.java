package dev.vesper.hud.agent;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;

/**
 * Adds one call at the start of the library's "present the frame" function:
 *   ((LongConsumer) System.getProperties().get(key)).accept(window)
 * Minecraft up to 1.21.x presents with LWJGL's GLFW.glfwSwapBuffers(long); Minecraft 26.x with SDL3's
 * SDLVideo.SDL_GL_SwapWindow(long). Only java.base types are named in the added code, so it works whichever
 * class loader or module holds LWJGL.
 */
final class GlfwPatch implements ClassFileTransformer {
    @Override
    public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain pd, byte[] bytes) {
        final String method, desc, key;
        if ("org/lwjgl/glfw/GLFW".equals(name)) { method = "glfwSwapBuffers"; desc = "(J)V"; key = Agent.GLFW_KEY; }
        else if ("org/lwjgl/sdl/SDLVideo".equals(name)) { method = "SDL_GL_SwapWindow"; desc = "(J)Z"; key = Agent.SDL_KEY; }
        else return null;
        if (loader == null) return null;
        try {
            Agent.gameLoader = loader;
            ClassReader cr = new ClassReader(bytes);
            ClassWriter cw = new ClassWriter(cr, ClassWriter.COMPUTE_MAXS);
            final boolean[] patched = { false };
            cr.accept(new ClassVisitor(Opcodes.ASM9, cw) {
                @Override
                public MethodVisitor visitMethod(int access, String mname, String mdesc, String sig, String[] ex) {
                    MethodVisitor mv = super.visitMethod(access, mname, mdesc, sig, ex);
                    if (!method.equals(mname) || !desc.equals(mdesc) || (access & Opcodes.ACC_STATIC) == 0) return mv;
                    patched[0] = true;
                    return new MethodVisitor(Opcodes.ASM9, mv) {
                        @Override
                        public void visitCode() {
                            super.visitCode();
                            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/System", "getProperties", "()Ljava/util/Properties;", false);
                            visitLdcInsn(key);
                            visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/util/Properties", "get", "(Ljava/lang/Object;)Ljava/lang/Object;", false);
                            visitTypeInsn(Opcodes.CHECKCAST, "java/util/function/LongConsumer");
                            visitVarInsn(Opcodes.LLOAD, 0);
                            visitMethodInsn(Opcodes.INVOKEINTERFACE, "java/util/function/LongConsumer", "accept", "(J)V", true);
                        }
                    };
                }
            }, 0);
            if (patched[0]) System.out.println("[Vesper HUD] hooked " + name.replace('/', '.') + "." + method + " (class loader: " + loader + ")");
            else System.err.println("[Vesper HUD] " + name + " was loaded but has no " + method + desc + "; the HUD cannot draw");
            return patched[0] ? cw.toByteArray() : null;
        } catch (Throwable t) {
            System.err.println("[Vesper HUD] could not patch " + name + ": " + t);
            return null;
        }
    }
}
