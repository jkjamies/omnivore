package com.jkjamies.omnivore.gradle.transform

import com.android.build.api.instrumentation.AsmClassVisitorFactory
import com.android.build.api.instrumentation.ClassContext
import com.android.build.api.instrumentation.ClassData
import com.android.build.api.instrumentation.InstrumentationParameters
import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.instrumentation.ClassInstrumenter
import com.jkjamies.omnivore.agent.instrumentation.ComposeDetector
import com.jkjamies.omnivore.agent.instrumentation.GlobPattern
import com.jkjamies.omnivore.agent.instrumentation.InstrumentingClassVisitor
import com.jkjamies.omnivore.agent.runtime.ClassId
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
import org.objectweb.asm.AnnotationVisitor
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
    val excludeAnnotations: ListProperty<String>

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

        // An explicit include overrides the built-in skip list, which contains
        // broad prefixes (`com.google.`, `com.squareup.`) that would otherwise
        // silently swallow a user's own code with no way to opt back in.
        val includes = parameters.get().includes.getOrElse(emptyList())
        val explicitlyIncluded =
            includes.isNotEmpty() && includes.any { patternMatches(it, className) }

        // Skip infrastructure
        if (!explicitlyIncluded && isInfrastructureClass(className)) return false

        // Check include patterns
        if (includes.isNotEmpty() && !explicitlyIncluded) return false

        // Check exclude patterns
        val excludes = parameters.get().excludes.getOrElse(emptyList())
        if (excludes.any { patternMatches(it, className) }) return false

        // Check annotation-based exclusions
        val excludeAnnotations = parameters.get().excludeAnnotations.getOrElse(emptyList())
        if (excludeAnnotations.isNotEmpty()) {
            val classAnnotations = classData.classAnnotations
            if (classAnnotations.any { annotation ->
                excludeAnnotations.any { pattern -> patternMatches(pattern, annotation) }
            }) return false
        }

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
            excludeAnnotations = parameters.get().excludeAnnotations.getOrElse(emptyList()),
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

    private fun patternMatches(pattern: String, text: String): Boolean =
        GlobPattern.matches(pattern, text)
}

/**
 * Buffers a class into an ASM [ClassNode], then instruments it with the same
 * [ClassInstrumenter] core the JVM agent uses.
 *
 * ## Why buffer instead of instrumenting as we go
 *
 * AGP hands the transform a streaming [ClassVisitor], and probe-array sizing
 * needs a number that is not known until every method has been seen. The
 * previous implementation instrumented in a single streaming pass and worked
 * around that by emitting a hardcoded `getProbes(classId, name, 1024)` from
 * `<clinit>` — "generous pre-allocation". Two things were wrong with it:
 *
 *  * A class with more than 1024 probes wrote past the end of its array and
 *    threw `ArrayIndexOutOfBoundsException` at runtime, inside the app under
 *    test. Edge-based branch probes roughly double probe counts, so that
 *    ceiling was going to start being hit.
 *  * Every instrumented class with a static initialiser allocated 1024 booleans
 *    regardless of need, and reported a probe count unrelated to reality.
 *
 * Buffering costs a `ClassNode` per class at build time and removes the guess
 * entirely. It also lets this path share the agent's probe walker, so switch
 * statements and edge probes are handled identically instead of being
 * reimplemented (and previously, omitted) here.
 */
private class OmnivoreInstrumentingVisitor(
    private val className: String,
    private val config: AgentConfig,
    private val probeMap: ProbeMap?,
    private val delegate: ClassVisitor,
) : ClassNode(Opcodes.ASM9) {

    override fun visitEnd() {
        super.visitEnd()

        // Interfaces cannot carry the ACC_TRANSIENT probe field.
        if ((access and Opcodes.ACC_INTERFACE) != 0) {
            accept(delegate)
            return
        }

        val totalProbeCount = ClassInstrumenter.countProbes(this, config)
        if (totalProbeCount == 0) {
            accept(delegate)
            return
        }

        val classId = ClassId.forClassName(className)
        val classProbeMap = probeMap?.getOrCreateClassMap(classId, className, sourceFile)
        if (classProbeMap != null) {
            ClassInstrumenter.buildProbeMap(this, config, classProbeMap)
        }

        // Replay the buffered class through the instrumenting visitor. The probe
        // map was built above, so pass null here — otherwise ProbeInserter would
        // record every probe a second time.
        accept(
            InstrumentingClassVisitor(
                classId = classId,
                className = className,
                classNode = this,
                config = config,
                totalProbeCount = totalProbeCount,
                classProbeMap = null,
                delegate = delegate,
            )
        )
    }
}
