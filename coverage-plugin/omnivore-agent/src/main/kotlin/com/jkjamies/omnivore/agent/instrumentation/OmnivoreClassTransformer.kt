package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.JumpInsnNode
import org.objectweb.asm.tree.LineNumberNode
import java.lang.instrument.ClassFileTransformer
import java.security.ProtectionDomain

/**
 * The core class transformer that instruments JVM classes with coverage probes.
 *
 * For each class that should be instrumented, it:
 * 1. Adds a static `$omnivoreProbes` boolean array field
 * 2. Generates/modifies `<clinit>` to call OmnivoreRuntime.getProbes() to initialize the array
 * 3. Inserts probe instructions at line boundaries and branch points in each method
 *
 * The probe array is shared with ExecutionDataStore via OmnivoreRuntime,
 * so probe hits are visible to the data store in real time.
 */
class OmnivoreClassTransformer(
    private val dataStore: ExecutionDataStore,
    private val probeMap: ProbeMap? = null,
    private val config: AgentConfig,
) : ClassFileTransformer {

    override fun transform(
        loader: ClassLoader?,
        className: String?,
        classBeingRedefined: Class<*>?,
        protectionDomain: ProtectionDomain?,
        classfileBuffer: ByteArray,
    ): ByteArray? {
        if (className == null) return null

        // Skip classes from classloaders that can't see OmnivoreRuntime.
        // Without this, instrumented classes would throw NoClassDefFoundError
        // when their <clinit> tries to call OmnivoreRuntime.getProbes().
        if (loader != null && !canSeeRuntime(loader)) return null

        // Never instrument JDK, Kotlin stdlib, or other infrastructure
        if (shouldSkipInfrastructure(className)) return null

        // Check include/exclude patterns
        if (!matchesIncludePatterns(className)) return null
        if (matchesExcludePatterns(className)) return null

        // Check Compose-generated class patterns
        if (config.composeFilterEnabled && ComposeDetector.isGeneratedClass(className)) return null
        if (config.composeFilterEnabled &&
            ComposeDetector.matchesExcludePattern(className, config.composeExcludePatterns)
        ) return null

        return try {
            instrumentClass(className, classfileBuffer)
        } catch (e: Exception) {
            System.err.println("[Omnivore] Warning: Failed to instrument $className: ${e.message}")
            null
        }
    }

    /**
     * Instrument a class:
     * 1. Analyze with tree API to count probes and make filtering decisions
     * 2. Instrument with visitor API, injecting probes and <clinit> initialization
     */
    private fun instrumentClass(className: String, classfileBuffer: ByteArray): ByteArray? {
        val reader = ClassReader(classfileBuffer)

        // First pass: analyze the class structure
        val classNode = ClassNode()
        reader.accept(classNode, ClassReader.EXPAND_FRAMES)

        // Check class-level Compose patterns with full class info
        if (config.composeFilterEnabled && ComposeDetector.isGeneratedClass(classNode)) {
            return null
        }

        // Count total probes needed across all methods
        val totalProbeCount = countProbes(classNode)
        if (totalProbeCount == 0) return null

        // Second pass: instrument
        val classId = classNameToId(className)
        val sourceFile = classNode.sourceFile
        val classProbeMap = probeMap?.getOrCreateClassMap(classId, className, sourceFile)
        val writer = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        val instrumenter = InstrumentingClassVisitor(
            classId = classId,
            className = className,
            classNode = classNode,
            config = config,
            totalProbeCount = totalProbeCount,
            classProbeMap = classProbeMap,
            delegate = writer,
        )

        reader.accept(instrumenter, ClassReader.EXPAND_FRAMES)
        return writer.toByteArray()
    }

    /** Count probes that will be inserted (dry run). */
    private fun countProbes(classNode: ClassNode): Int {
        var total = 0
        for (method in classNode.methods ?: emptyList()) {
            val name = method.name ?: continue
            val access = method.access
            if (name == "<clinit>") continue
            if ((access and Opcodes.ACC_BRIDGE) != 0) continue
            if ((access and Opcodes.ACC_ABSTRACT) != 0) continue
            if ((access and Opcodes.ACC_NATIVE) != 0) continue
            if (KotlinDetector.isSyntheticBridgeMethod(method)) continue
            if (config.composeFilterEnabled && ComposeDetector.isComposeLambdaGroup(name)) continue

            val seenLines = mutableSetOf<Int>()
            for (insn in method.instructions ?: continue) {
                when (insn.type) {
                    AbstractInsnNode.LINE -> {
                        if (seenLines.add((insn as LineNumberNode).line)) total++
                    }
                    AbstractInsnNode.JUMP_INSN -> {
                        if ((insn as JumpInsnNode).opcode != Opcodes.GOTO) total++
                    }
                }
            }
        }
        return total
    }

    /**
     * Check if a classloader can see OmnivoreRuntime.
     * Only instrument classes from classloaders that can resolve the runtime,
     * otherwise the instrumented <clinit> will throw NoClassDefFoundError.
     */
    private fun canSeeRuntime(loader: ClassLoader): Boolean {
        return try {
            loader.loadClass("com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime")
            true
        } catch (_: ClassNotFoundException) {
            false
        }
    }

    private fun shouldSkipInfrastructure(className: String): Boolean {
        val skipPrefixes = arrayOf(
            // JDK
            "java/", "javax/", "jdk/", "sun/",
            // Kotlin
            "kotlin/", "kotlinx/",
            // Build tools & test frameworks
            "org/gradle/", "worker/",
            "org/junit/", "org/hamcrest/", "org/assertj/", "org/mockito/",
            "org/testng/", "io/mockk/",
            // Gradle internal dependencies
            "com/esotericsoftware/", "org/objenesis/",
            // Logging
            "org/slf4j/", "ch/qos/logback/", "org/apache/logging/",
            "org/apache/log4j/",
            // Common libraries
            "org/objectweb/asm/",
            "org/apache/commons/", "org/apache/http/",
            "com/google/", "io/netty/", "io/grpc/",
            "com/fasterxml/", "com/squareup/",
            "org/jetbrains/annotations/",
            // Android / Compose
            "android/", "dalvik/",
            "androidx/compose/runtime/",
            "androidx/compose/ui/",
            "androidx/compose/foundation/",
            "androidx/compose/material",
            "androidx/annotation/", "androidx/collection/",
            // Our own agent
            "com/jkjamies/omnivore/agent/",
        )
        return skipPrefixes.any { className.startsWith(it) }
    }

    private fun matchesIncludePatterns(className: String): Boolean {
        if (config.includes.isEmpty()) return true
        val dotName = className.replace('/', '.')
        return config.includes.any { globMatches(it, dotName) }
    }

    private fun matchesExcludePatterns(className: String): Boolean {
        val dotName = className.replace('/', '.')
        return config.excludes.any { globMatches(it, dotName) }
    }

    private fun globMatches(pattern: String, text: String): Boolean {
        val regex = pattern
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", ".")
        return Regex(regex).matches(text)
    }

    companion object {
        fun classNameToId(className: String): Long {
            var hash = 0L
            for (char in className) {
                hash = hash * 31 + char.code
            }
            return hash
        }
    }
}

/**
 * ASM ClassVisitor that instruments methods with coverage probes
 * and generates the probe initialization code in <clinit>.
 */
private class InstrumentingClassVisitor(
    private val classId: Long,
    private val className: String,
    private val classNode: ClassNode,
    private val config: AgentConfig,
    private val totalProbeCount: Int,
    private val classProbeMap: ClassProbeMap?,
    delegate: ClassVisitor,
) : ClassVisitor(Opcodes.ASM9, delegate) {

    private var globalProbeOffset = 0
    private var hasExistingClinit = false

    override fun visitMethod(
        access: Int,
        name: String?,
        descriptor: String?,
        signature: String?,
        exceptions: Array<out String>?,
    ): MethodVisitor? {
        if (name == null || descriptor == null) {
            return super.visitMethod(access, name, descriptor, signature, exceptions)
        }

        // Prepend probe initialization to existing <clinit>
        if (name == "<clinit>") {
            hasExistingClinit = true
            val mv = super.visitMethod(access, name, descriptor, signature, exceptions)
                ?: return null
            return ClinitPrefixVisitor(classId, className, totalProbeCount, mv)
        }

        val mv = super.visitMethod(access, name, descriptor, signature, exceptions) ?: return null

        // Skip non-instrumentable methods
        if ((access and Opcodes.ACC_BRIDGE) != 0) return mv
        if ((access and Opcodes.ACC_ABSTRACT) != 0) return mv
        if ((access and Opcodes.ACC_NATIVE) != 0) return mv

        val methodNode = classNode.methods?.find { it.name == name && it.desc == descriptor }
        if (methodNode != null && KotlinDetector.isSyntheticBridgeMethod(methodNode)) return mv
        if (config.composeFilterEnabled && ComposeDetector.isComposeLambdaGroup(name)) return mv

        val currentOffset = globalProbeOffset
        val probeInserter = ProbeInserter(className, currentOffset, name, descriptor, classProbeMap, mv)
        return ProbeCountingMethodVisitor(probeInserter) { count ->
            globalProbeOffset += count
        }
    }

    override fun visitEnd() {
        // Add the $omnivoreProbes static field
        super.visitField(
            Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC or Opcodes.ACC_SYNTHETIC or Opcodes.ACC_TRANSIENT,
            ProbeInserter.PROBE_FIELD_NAME,
            ProbeInserter.PROBE_FIELD_DESCRIPTOR,
            null,
            null
        )?.visitEnd()

        // Generate <clinit> if the class doesn't have one
        if (!hasExistingClinit) {
            val mv = super.visitMethod(Opcodes.ACC_STATIC, "<clinit>", "()V", null, null)
            if (mv != null) {
                mv.visitCode()
                emitProbeInit(mv, classId, className, totalProbeCount)
                mv.visitInsn(Opcodes.RETURN)
                mv.visitMaxs(3, 0)
                mv.visitEnd()
            }
        }

        super.visitEnd()
    }
}

/** Prepends probe initialization to an existing <clinit>. */
private class ClinitPrefixVisitor(
    private val classId: Long,
    private val className: String,
    private val totalProbeCount: Int,
    delegate: MethodVisitor,
) : MethodVisitor(Opcodes.ASM9, delegate) {
    override fun visitCode() {
        super.visitCode()
        emitProbeInit(mv, classId, className, totalProbeCount)
    }
}

/** Wraps a ProbeInserter to capture its final count after visitation. */
private class ProbeCountingMethodVisitor(
    private val probeInserter: ProbeInserter,
    private val onEnd: (Int) -> Unit,
) : MethodVisitor(Opcodes.ASM9, probeInserter) {
    override fun visitEnd() {
        super.visitEnd()
        onEnd(probeInserter.probeCount)
    }
}

/**
 * Emit bytecode: $omnivoreProbes = OmnivoreRuntime.getProbes(classId, className, probeCount)
 */
private fun emitProbeInit(mv: MethodVisitor, classId: Long, className: String, probeCount: Int) {
    mv.visitLdcInsn(classId)
    mv.visitLdcInsn(className.replace('/', '.'))
    emitIntPush(mv, probeCount)
    mv.visitMethodInsn(
        Opcodes.INVOKESTATIC,
        OmnivoreRuntime.INTERNAL_NAME,
        OmnivoreRuntime.GET_PROBES_METHOD,
        OmnivoreRuntime.GET_PROBES_DESCRIPTOR,
        false
    )
    mv.visitFieldInsn(
        Opcodes.PUTSTATIC,
        className,
        ProbeInserter.PROBE_FIELD_NAME,
        ProbeInserter.PROBE_FIELD_DESCRIPTOR
    )
}

private fun emitIntPush(mv: MethodVisitor, value: Int) {
    when {
        value in -1..5 -> mv.visitInsn(Opcodes.ICONST_0 + value)
        value in Byte.MIN_VALUE..Byte.MAX_VALUE -> mv.visitIntInsn(Opcodes.BIPUSH, value)
        value in Short.MIN_VALUE..Short.MAX_VALUE -> mv.visitIntInsn(Opcodes.SIPUSH, value)
        else -> mv.visitLdcInsn(value)
    }
}
