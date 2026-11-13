package dev.vesper.hud.agent;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.lang.instrument.ClassFileTransformer;
import java.security.ProtectionDomain;

/**
 * Adds one call at the start of GLFW.glfwSwapBuffers(long):
 *   ((LongConsumer) System.getProperties().get("vesper.hud.swap")).accept(window)
 * Only java.base types are named, so it works whichever class loader or module holds LWJGL.
 */
final class GlfwPatch implements ClassFileTransformer {
    @Override
    public byte[] transform(ClassLoader loader, String name, Class<?> redefined, ProtectionDomain pd, byte[] bytes) {
        if (!"org/lwjgl/glfw/GLFW".equals(name) || loader == null) return null;
        try {
            Agent.gameLoader = loader;
            ClassReader cr = new ClassReader(bytes);
            ClassWriter cw = new ClassWriter(cr, ClassWriter.COMPUTE_MAXS);
            final boolean[] patched = { false };
            cr.accept(new ClassVisitor(Opcodes.ASM9, cw) {
                @Override
                public MethodVisitor visitMethod(int access, String mname, String desc, String sig, String[] ex) {
                    MethodVisitor mv = super.visitMethod(access, mname, desc, sig, ex);
                    if (!"glfwSwapBuffers".equals(mname) || !"(J)V".equals(desc) || (access & Opcodes.ACC_STATIC) == 0) return mv;
                    patched[0] = true;
                    return new MethodVisitor(Opcodes.ASM9, mv) {
                        @Override
                        public void visitCode() {
                            super.visitCode();
                            visitMethodInsn(Opcodes.INVOKESTATIC, "java/lang/System", "getProperties", "()Ljava/util/Properties;", false);
                            visitLdcInsn(Agent.HOOK_KEY);
                            visitMethodInsn(Opcodes.INVOKEVIRTUAL, "java/util/Properties", "get", "(Ljava/lang/Object;)Ljava/lang/Object;", false);
                            visitTypeInsn(Opcodes.CHECKCAST, "java/util/function/LongConsumer");
                            visitVarInsn(Opcodes.LLOAD, 0);
                            visitMethodInsn(Opcodes.INVOKEINTERFACE, "java/util/function/LongConsumer", "accept", "(J)V", true);
                        }
                    };
                }
            }, 0);
            if (patched[0]) System.out.println("[Vesper HUD] hooked GLFW.glfwSwapBuffers (class loader: " + loader + ")");
            else System.err.println("[Vesper HUD] GLFW was loaded but has no glfwSwapBuffers(long); the HUD cannot draw");
            return patched[0] ? cw.toByteArray() : null;
        } catch (Throwable t) {
            System.err.println("[Vesper HUD] could not patch GLFW: " + t);
            return null;
        }
    }
}
