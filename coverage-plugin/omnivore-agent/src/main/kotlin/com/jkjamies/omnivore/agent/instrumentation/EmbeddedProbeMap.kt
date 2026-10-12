package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.EmbeddedProbeMapData
import com.jkjamies.omnivore.agent.runtime.ProbeMapReader
import com.jkjamies.omnivore.agent.runtime.ProbeMapWriter
import org.objectweb.asm.AnnotationVisitor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.Type
import org.objectweb.asm.tree.AnnotationNode
import org.objectweb.asm.tree.ClassNode
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.util.Base64

/**
 * Stores each class's probe map inside the instrumented class, so the build can
 * read the map back from exactly the bytecode that gets dexed.
 *
 * ## Why not collect the map during the transform
 *
 * AGP runs the transform incrementally, caches it across `clean` (dependency
 * modules go through a Gradle artifact transform with a global cache), and may
 * skip it entirely. A map gathered as a side effect of the transform therefore
 * only ever saw the classes that happened to be transformed in this JVM: after a
 * one-file change the rig's map held 13 of its 25 app classes, and classes from
 * dependency modules — instrumented, and recording hits on device — were never
 * in it at all. Their coverage was silently dropped.
 *
 * Whatever the transform's outputs are, they always carry the annotation, so
 * reading the outputs gives the complete map no matter which classes this
 * particular build actually re-transformed.
 */
object EmbeddedProbeMap {

    val DESCRIPTOR: String = Type.getDescriptor(EmbeddedProbeMapData::class.java)

    /** Constant-pool strings are capped at 65535 bytes; stay well under it. */
    private const val CHUNK_SIZE = 60_000

    /** Adds [classMap] to [classNode] as an [EmbeddedProbeMapData] annotation. */
    fun attach(classNode: ClassNode, classMap: ClassProbeMap) {
        val bytes = ByteArrayOutputStream().also { ProbeMapWriter.write(it, listOf(classMap)) }.toByteArray()
        val encoded = Base64.getEncoder().encodeToString(bytes)
        val annotation = AnnotationNode(DESCRIPTOR)
        annotation.values = listOf("chunks", encoded.chunked(CHUNK_SIZE))
        val existing = classNode.invisibleAnnotations ?: mutableListOf()
        existing.removeAll { it.desc == DESCRIPTOR }
        existing.add(annotation)
        classNode.invisibleAnnotations = existing
    }

    /** The probe map embedded in [classBytes], or null if the class carries none. */
    fun read(classBytes: ByteArray): ClassProbeMap? {
        val chunks = mutableListOf<String>()
        var found = false
        ClassReader(classBytes).accept(
            object : ClassVisitor(Opcodes.ASM9) {
                override fun visitAnnotation(descriptor: String, visible: Boolean): AnnotationVisitor? {
                    if (descriptor != DESCRIPTOR) return null
                    found = true
                    return object : AnnotationVisitor(Opcodes.ASM9) {
                        override fun visitArray(name: String?): AnnotationVisitor =
                            object : AnnotationVisitor(Opcodes.ASM9) {
                                override fun visit(name: String?, value: Any?) {
                                    if (value is String) chunks += value
                                }
                            }
                    }
                }
            },
            ClassReader.SKIP_CODE or ClassReader.SKIP_DEBUG or ClassReader.SKIP_FRAMES,
        )
        if (!found) return null
        val bytes = Base64.getDecoder().decode(chunks.joinToString(""))
        val map = ProbeMapReader.read(ByteArrayInputStream(bytes), "embedded probe map")
        return map.getAllClassMaps().singleOrNull()
    }
}
