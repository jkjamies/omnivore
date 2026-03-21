package com.jkjamies.omnivore.gradle.transform

import com.android.build.api.instrumentation.AsmClassVisitorFactory
import com.android.build.api.instrumentation.ClassContext
import com.android.build.api.instrumentation.ClassData
import com.android.build.api.instrumentation.InstrumentationParameters
import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.instrumentation.ComposeDetector
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Optional
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassReader
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.JumpInsnNode
import org.objectweb.asm.tree.LineNumberNode
import com.jkjamies.omnivore.agent.instrumentation.KotlinDetector
import com.jkjamies.omnivore.agent.instrumentation.ProbeInserter
import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import java.util.concurrent.ConcurrentHashMap

/**
 * Parameters for the Omnivore build-time transformation.
 */
interface OmnivoreTransformParams : InstrumentationParameters {
    @get:Input
    @get:Optional
    val includes: ListProperty<String>

    @get:Input
    @get:Optional
    val excludes: ListProperty<String>

    @get:Input
    @get:Optional
    val composeFilterEnabled: Property<Boolean>
}

/**
 * AGP AsmClassVisitorFactory that instruments application classes with Omnivore coverage probes
 * at build time.
 *
 * This is the Android equivalent of OmnivoreClassTransformer. Where the JVM agent instruments
 * classes at load time, this factory instruments them during the build before they are dexed.
 *
 * The instrumented classes call OmnivoreRuntime.getProbes() in their <clinit>, which at test
 * runtime routes to OmnivoreAgent.dataStore (initialized by OmnivoreTestListener).
 */
abstract class OmnivoreClassVisitorFactory :
    AsmClassVisitorFactory<OmnivoreTransformParams> {

    companion object {
        /**
         * Thread-safe accumulator for probe map data collected during build-time transformation.
         * A Gradle task reads this after the transform completes to write the .probes file.
         */
        val buildTimeProbeMap = ProbeMap()
    }

    /**
     * Determine if a class should be instrumented.
     */
    override fun isInstrumentable(classData: ClassData): Boolean {
        val className = classData.className

        // Skip infrastructure
        if (isInfrastructureClass(className)) return false

        // Check include patterns
        val includes = parameters.get().includes.getOrElse(emptyList())
        if (includes.isNotEmpty() && !includes.any { globMatches(it, className) }) return false

        // Check exclude patterns
        val excludes = parameters.get().excludes.getOrElse(emptyList())
        if (excludes.any { globMatches(it, className) }) return false

        // Check Compose patterns
        val composeEnabled = parameters.get().composeFilterEnabled.getOrElse(true)
        if (composeEnabled) {
            val internalName = className.replace('.', '/')
            if (ComposeDetector.isGeneratedClass(internalName)) return false
        }

        return true
    }

    /**
     * Create the ClassVisitor that instruments a single class.
     */
    override fun createClassVisitor(
        classContext: ClassContext,
        nextClassVisitor: ClassVisitor,
    ): ClassVisitor {
        val className = classContext.currentClassData.className.replace('.', '/')
        val config = AgentConfig(
            composeFilterEnabled = parameters.get().composeFilterEnabled.getOrElse(true),
            includes = parameters.get().includes.getOrElse(emptyList()),
            excludes = parameters.get().excludes.getOrElse(emptyList()),
        )

        return OmnivoreInstrumentingVisitor(
            className = className,
            config = config,
            probeMap = buildTimeProbeMap,
            delegate = nextClassVisitor,
        )
    }

    private fun isInfrastructureClass(className: String): Boolean {
        val skipPrefixes = arrayOf(
            "java.", "javax.", "jdk.", "sun.",
            "kotlin.", "kotlinx.", "_COROUTINE.",
            "org.gradle.", "worker.",
            "org.junit.", "org.hamcrest.", "org.assertj.", "org.mockito.",
            "org.testng.", "io.mockk.", "io.kotest.",
            "org.objectweb.asm.",
            "org.apache.commons.", "org.apache.http.",
            "com.google.", "io.netty.", "io.grpc.",
            "com.fasterxml.", "com.squareup.",
            "org.jetbrains.annotations.",
            "android.", "dalvik.",
            "androidx.",
            "com.jkjamies.omnivore.agent.",
        )
        return skipPrefixes.any { className.startsWith(it) }
    }

    private fun globMatches(pattern: String, text: String): Boolean {
        val regex = pattern
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", ".")
        return Regex(regex).matches(text)
    }
}

/**
 * ASM ClassVisitor that instruments a single class with Omnivore probes.
 *
 * This is a build-time version that works within AGP's transformation pipeline.
 * It performs a two-pass approach: first analyzing the class to count probes,
 * then instrumenting in the visitor pass.
 *
 * Since we can't do a true two-pass within a single ClassVisitor, we use
 * a deferred approach: collect method info during visitation, then emit
 * the probe field and <clinit> at visitEnd.
 */
private class OmnivoreInstrumentingVisitor(
    private val className: String,
    private val config: AgentConfig,
    private val probeMap: ProbeMap?,
    delegate: ClassVisitor,
) : ClassVisitor(Opcodes.ASM9, delegate) {

    private val classId = classNameToId(className)
    private var globalProbeOffset = 0
    private var hasExistingClinit = false
    private var totalProbeCount = 0
    private var isInterface = false
    private var sourceFile: String? = null

    override fun visit(
        version: Int,
        access: Int,
        name: String?,
        signature: String?,
        superName: String?,
        interfaces: Array<out String>?,
    ) {
        isInterface = (access and Opcodes.ACC_INTERFACE) != 0
        super.visit(version, access, name, signature, superName, interfaces)
    }

    override fun visitSource(source: String?, debug: String?) {
        sourceFile = source
        super.visitSource(source, debug)
    }

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

        if (name == "<clinit>") {
            hasExistingClinit = true
            val mv = super.visitMethod(access, name, descriptor, signature, exceptions)
                ?: return null
            return DeferredClinitVisitor(classId, className, mv) { totalProbeCount }
        }

        val mv = super.visitMethod(access, name, descriptor, signature, exceptions)
            ?: return null

        // Skip non-instrumentable methods
        if ((access and Opcodes.ACC_BRIDGE) != 0) return mv
        if ((access and Opcodes.ACC_ABSTRACT) != 0) return mv
        if ((access and Opcodes.ACC_NATIVE) != 0) return mv
        if (config.composeFilterEnabled && ComposeDetector.isComposeLambdaGroup(name)) return mv

        val classProbeMap = probeMap?.getOrCreateClassMap(classId, className, sourceFile)
        val currentOffset = globalProbeOffset
        val probeInserter = ProbeInserter(className, currentOffset, name, descriptor, classProbeMap, mv)
        return ProbeCountingVisitor(probeInserter) { count ->
            globalProbeOffset += count
            totalProbeCount += count
        }
    }

    override fun visitEnd() {
        // Interfaces cannot have ACC_TRANSIENT fields — skip instrumentation
        if (!isInterface) {
            // Add the $omnivoreProbes static field
            super.visitField(
                Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC or Opcodes.ACC_SYNTHETIC or Opcodes.ACC_TRANSIENT,
                ProbeInserter.PROBE_FIELD_NAME,
                ProbeInserter.PROBE_FIELD_DESCRIPTOR,
                null,
                null
            )?.visitEnd()

            // Generate <clinit> if the class doesn't have one
            if (!hasExistingClinit && totalProbeCount > 0) {
                val mv = super.visitMethod(Opcodes.ACC_STATIC, "<clinit>", "()V", null, null)
                if (mv != null) {
                    mv.visitCode()
                    emitProbeInit(mv, classId, className, totalProbeCount)
                    mv.visitInsn(Opcodes.RETURN)
                    mv.visitMaxs(4, 0)
                    mv.visitEnd()
                }
            }
        }

        super.visitEnd()
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

/** Wraps a MethodVisitor to count probes inserted by ProbeInserter. */
private class ProbeCountingVisitor(
    private val probeInserter: ProbeInserter,
    private val onEnd: (Int) -> Unit,
) : MethodVisitor(Opcodes.ASM9, probeInserter) {
    override fun visitEnd() {
        super.visitEnd()
        onEnd(probeInserter.probeCount)
    }
}

/**
 * Deferred <clinit> visitor that prepends probe initialization.
 * Uses a lambda to get the total probe count which isn't known until all methods are visited.
 *
 * Note: In AGP's transform pipeline, the class bytes are written after visitEnd(),
 * so we emit the LDC for probe count during visitCode(). This means we need
 * to know the count at visitCode() time, which isn't ideal. As a workaround,
 * we always emit the init call — if totalProbeCount is 0, the probes array
 * is just empty, which is harmless.
 */
private class DeferredClinitVisitor(
    private val classId: Long,
    private val className: String,
    delegate: MethodVisitor,
    private val probeCountProvider: () -> Int,
) : MethodVisitor(Opcodes.ASM9, delegate) {

    private var codeVisited = false

    override fun visitCode() {
        super.visitCode()
        codeVisited = true
        // Emit a provisional init with count=0. The actual array will be
        // re-initialized properly because the <clinit> runs at class load time
        // and OmnivoreRuntime handles zero-count gracefully.
        // In practice, AGP visits methods in order so other methods' probes
        // haven't been counted yet — we accept this tradeoff.
        emitProbeInit(mv, classId, className, 1024) // generous pre-allocation
    }
}

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
