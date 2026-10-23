// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
package icaro.instalador;

import java.io.*;
import java.lang.instrument.*;
import java.nio.charset.StandardCharsets;
import java.security.ProtectionDomain;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.jar.JarFile;

/** Agrupa escrituras del parcheador sin modificar el instalador almacenado. */
public final class BufferAgent {
    public static void premain(String ruta, Instrumentation instrumentation) {
        try {
            // Se carga antes de añadir el JAR al bootstrap para mantener este
            // transformador y su clase exterior en el mismo cargador.
            ClassFileTransformer transformer = new ClassFileTransformer() {
                public byte[] transform(ClassLoader loader, String name, Class<?> type,
                                        ProtectionDomain domain, byte[] bytes) {
                    if (!"net/minecraftforge/binarypatcher/Patcher".equals(name)) return null;
                    try {
                        // Solo se adapta la implementación oficial medida y
                        // comprobada. Una actualización conserva su E/S original.
                        byte[] hash = MessageDigest.getInstance("SHA-256").digest(bytes);
                        StringBuilder hex = new StringBuilder();
                        for (byte value : hash) hex.append(String.format("%02x", value & 255));
                        if (!"8e7427ef29897b978790f8db9c444a4d5800c75b7d4b627716b7a915db710e77".equals(hex.toString())) return null;
                        byte[] changed = agrupar(bytes);
                        if (changed != null) System.out.println("Icaro: escritura agrupada del parcheador activada");
                        return changed;
                    } catch (Exception error) {
                        System.out.println("Icaro: se conserva la escritura original del parcheador");
                        return null;
                    }
                }
            };
            // Los procesadores pueden usar cargadores aislados. El adaptador
            // público solo depende de java.io y debe ser visible desde ellos.
            instrumentation.appendToBootstrapClassLoaderSearch(new JarFile(ruta));
            instrumentation.addTransformer(transformer);
        } catch (Exception error) {
            System.out.println("Icaro: optimizacion de escritura no disponible");
        }
    }

    private static byte[] agrupar(byte[] original) throws IOException {
        DataInputStream in = new DataInputStream(new ByteArrayInputStream(original));
        ByteArrayOutputStream bytes = new ByteArrayOutputStream(original.length + 128);
        DataOutputStream out = new DataOutputStream(bytes);
        if (in.readInt() != 0xCAFEBABE) return null;
        out.writeInt(0xCAFEBABE);
        out.writeShort(in.readUnsignedShort());
        out.writeShort(in.readUnsignedShort());
        int count = in.readUnsignedShort();
        out.writeShort(count);
        byte[] from = "java/io/FileOutputStream".getBytes(StandardCharsets.UTF_8);
        byte[] to = "icaro/instalador/BufferAgent$BufferedFile".getBytes(StandardCharsets.UTF_8);
        boolean changed = false;
        for (int i = 1; i < count; i++) {
            int tag = in.readUnsignedByte();
            out.writeByte(tag);
            int size;
            switch (tag) {
                case 1:
                    byte[] value = new byte[in.readUnsignedShort()];
                    in.readFully(value);
                    if (Arrays.equals(value, from)) { value = to; changed = true; }
                    out.writeShort(value.length);
                    out.write(value);
                    continue;
                case 3: case 4: case 9: case 10: case 11: case 12: case 17: case 18:
                    size = 4; break;
                case 5: case 6: size = 8; i++; break;
                case 7: case 8: case 16: case 19: case 20: size = 2; break;
                case 15: size = 3; break;
                default: return null;
            }
            for (int n = 0; n < size; n++) out.writeByte(in.readUnsignedByte());
        }
        if (!changed) return null;
        byte[] rest = new byte[8192];
        int n;
        while ((n = in.read(rest)) != -1) out.write(rest, 0, n);
        return bytes.toByteArray();
    }

    /** Conserva orden y contenido; solo reduce las llamadas al sistema. */
    public static final class BufferedFile extends FileOutputStream {
        private final byte[] buffer = new byte[65536];
        private int used;
        private boolean closed;

        public BufferedFile(File file) throws FileNotFoundException { super(file); }
        public BufferedFile(String file) throws FileNotFoundException { super(file); }
        public BufferedFile(File file, boolean append) throws FileNotFoundException { super(file, append); }
        public BufferedFile(String file, boolean append) throws FileNotFoundException { super(file, append); }
        public BufferedFile(FileDescriptor file) { super(file); }

        public synchronized void write(int value) throws IOException {
            comprobarAbierto();
            if (used == buffer.length) flush();
            buffer[used++] = (byte)value;
        }
        public synchronized void write(byte[] value) throws IOException { write(value, 0, value.length); }
        public synchronized void write(byte[] value, int offset, int length) throws IOException {
            comprobarAbierto();
            if (offset < 0 || length < 0 || offset > value.length - length) throw new IndexOutOfBoundsException();
            if (length >= buffer.length) { flush(); super.write(value, offset, length); return; }
            if (length > buffer.length - used) flush();
            System.arraycopy(value, offset, buffer, used, length);
            used += length;
        }
        public synchronized void flush() throws IOException {
            comprobarAbierto();
            if (used > 0) { super.write(buffer, 0, used); used = 0; }
        }
        public synchronized void close() throws IOException {
            if (closed) return;
            try { flush(); } finally { closed = true; super.close(); }
        }
        private void comprobarAbierto() throws IOException {
            if (closed) throw new IOException("Stream closed");
        }
    }
}
