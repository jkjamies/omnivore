package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.runtime.ClassId
import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeEntry
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.LineNumberNode
import org.objectweb.asm.tree.LookupSwitchInsnNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.TableSwitchInsnNode
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
        if (loader != null && !canSeeRuntime(loader)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.NO_RUNTIME_ACCESS)
            return null
        }

        // Skip classes from test source sets (e.g., build/classes/kotlin/test/)
        if (isFromTestSourceSet(protectionDomain)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.TEST_SOURCES)
            return null
        }

        // An explicit include wins over the built-in skip list. The list is a
        // convenience for the common case, not a statement about what can be
        // instrumented — and it contains broad prefixes like `com/google/` and
        // `com/squareup/`. Anyone whose own code lives under one of those used
        // to get no coverage and no explanation, with no way to override it.
        val explicitlyIncluded = matchesIncludePatterns(className, requireExplicit = true)

        // Never instrument JDK, Kotlin stdlib, or other infrastructure
        if (!explicitlyIncluded && shouldSkipInfrastructure(className)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.INFRASTRUCTURE)
            return null
        }

        // Check include/exclude patterns. An exclude still wins over an
        // include — that is the user contradicting themselves, and the safer
        // reading of "exclude this" is to honour it.
        if (!matchesIncludePatterns(className) || matchesExcludePatterns(className)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.FILTERED)
            return null
        }

        // Check Compose-generated class patterns
        if (config.composeFilterEnabled &&
            (ComposeDetector.isGeneratedClass(className) ||
                ComposeDetector.matchesExcludePattern(className, config.composeExcludePatterns))
        ) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.COMPOSE)
            return null
        }

        return try {
            instrumentClass(className, classfileBuffer, loader).also {
                if (it != null) InstrumentationStats.instrumented()
            }
        } catch (e: Exception) {
            InstrumentationStats.failed(className, e)
            System.err.println("[Omnivore] Warning: Failed to instrument $className: ${e.message}")
            null
        }
    }

    /**
     * Instrument a class:
     * 1. Analyze with tree API to count probes and make filtering decisions
     * 2. Instrument with visitor API, injecting probes and <clinit> initialization
     */
    private fun instrumentClass(
        className: String,
        classfileBuffer: ByteArray,
        loader: ClassLoader?,
    ): ByteArray? {
        val reader = ClassReader(classfileBuffer)

        // First pass: analyze the class structure
        val classNode = ClassNode()
        reader.accept(classNode, ClassReader.EXPAND_FRAMES)

        // Skip interfaces — adding static fields with ACC_TRANSIENT to interfaces is illegal
        if ((classNode.access and Opcodes.ACC_INTERFACE) != 0) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.INTERFACE)
            return null
        }

        // If already instrumented by AGP build-time transform, don't re-instrument
        // but still build the probe map so the report task can correlate probes to source lines.
        // The <clinit> already calls OmnivoreRuntime.getProbes() which registers the probe
        // array with ExecutionDataStore at class load time — execution data is collected automatically.
        val alreadyInstrumented = classNode.fields?.any { it.name == ProbeInserter.PROBE_FIELD_NAME } == true

        // Check class-level Compose patterns with full class info
        if (config.composeFilterEnabled && ComposeDetector.isGeneratedClass(classNode)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.COMPOSE)
            return null
        }

        // Check annotation-based exclusion
        if (hasExcludedAnnotation(classNode.visibleAnnotations, classNode.invisibleAnnotations)) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.FILTERED)
            return null
        }

        // Count total probes needed across all methods
        val totalProbeCount = ClassInstrumenter.countProbes(classNode, config)
        if (totalProbeCount == 0) {
            // Overwhelmingly means the class was compiled without debug info,
            // since line probes come from the line-number table.
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.NO_LINE_NUMBERS)
            return null
        }

        // Build the probe map (needed for both fresh and already-instrumented classes)
        val classId = classNameToId(className)
        val sourceFile = classNode.sourceFile
        val classProbeMap = probeMap?.getOrCreateClassMap(classId, className, sourceFile)
        if (classProbeMap != null) {
            ClassInstrumenter.buildProbeMap(classNode, config, classProbeMap)
        }

        // If already instrumented, we have the probe map now — don't re-instrument
        if (alreadyInstrumented) {
            InstrumentationStats.skipped(InstrumentationStats.SkipReason.ALREADY_INSTRUMENTED)
            return null
        }

        // Second pass: instrument.
        //
        // classProbeMap is deliberately null here: buildProbeMap above already
        // recorded an entry for every probe. Passing it again made ProbeInserter
        // record each probe a *second* time, which doubled every class's probe
        // list — and since CoverageAnalyzer counts one branch per BRANCH entry,
        // it doubled reported branch totals. (Line entries survived because they
        // are keyed by line number and collapsed on insert, which is why the
        // duplication went unnoticed.)
        val writer = LoaderAwareClassWriter(loader)
        val instrumenter = InstrumentingClassVisitor(
            classId = classId,
            className = className,
            classNode = classNode,
            config = config,
            totalProbeCount = totalProbeCount,
            classProbeMap = null,
            delegate = writer,
        )

        reader.accept(instrumenter, ClassReader.EXPAND_FRAMES)
        return writer.toByteArray()
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

    /**
     * Check if a class originates from a test source set by inspecting its code source location.
     *
     * Standard Gradle: build/classes/{language}/{sourceSet}/
     * AGP (Android):   build/tmp/kotlin-classes/{sourceSet}/
     *
     * Test source sets: test, testDebug, testRelease, debugUnitTest, releaseUnitTest,
     * androidTest, instrumentedTest, etc.
     */
    private fun isFromTestSourceSet(protectionDomain: ProtectionDomain?): Boolean {
        val location = protectionDomain?.codeSource?.location?.path ?: return false
        return TEST_SOURCE_SET_PATTERN.containsMatchIn(location)
    }

    companion object {
        /** Matches Gradle and AGP test output directories. */
        private val TEST_SOURCE_SET_PATTERN = Regex(
            "/(classes/[^/]+|kotlin-classes)/((test|androidTest|instrumentedTest)[^/]*|[^/]*(UnitTest|AndroidTest))(/|$)"
        )

        /**
         * Class identifier. Delegates to [ClassId] so the JVM agent and the AGP
         * build-time transform derive the same value — they must, because the
         * agent builds probe maps for classes the AGP path already instrumented,
         * and the ID is baked into that bytecode.
         */
        fun classNameToId(className: String): Long = ClassId.forClassName(className)
    }

    private fun shouldSkipInfrastructure(className: String): Boolean {
        val skipPrefixes = arrayOf(
            // JDK
            "java/", "javax/", "jdk/", "sun/",
            // Kotlin
            "kotlin/", "kotlinx/", "_COROUTINE/",
            // Build tools & test frameworks
            "org/gradle/", "worker/",
            "org/junit/", "org/hamcrest/", "org/assertj/", "org/mockito/",
            "org/testng/", "io/mockk/", "io/kotest/",
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
            // Android / AndroidX / Compose
            "android/", "dalvik/",
            "androidx/",
            // Our own agent
            "com/jkjamies/omnivore/agent/",
        )
        return skipPrefixes.any { className.startsWith(it) }
    }

    /**
     * @param requireExplicit when true, an empty include list does *not* match.
     *   Used to decide whether the user has deliberately opted a class in,
     *   which is what allows overriding the built-in infrastructure skip list.
     */
    private fun matchesIncludePatterns(className: String, requireExplicit: Boolean = false): Boolean {
        if (config.includes.isEmpty()) return !requireExplicit
        val dotName = className.replace('/', '.')
        return config.includes.any { patternMatches(it, dotName) }
    }

    private fun matchesExcludePatterns(className: String): Boolean {
        val dotName = className.replace('/', '.')
        return config.excludes.any { patternMatches(it, dotName) }
    }

    /**
     * Match a pattern against text. Supports glob (default) and regex (prefix with "regex:").
     */
    private fun patternMatches(pattern: String, text: String): Boolean =
        GlobPattern.matches(pattern, text)

    /**
     * Check if any of the annotations match the configured exclude annotation patterns.
     * Annotation descriptors use the format "Lcom/example/MyAnnotation;" — we convert
     * to dot-notation for matching.
     */
    private fun hasExcludedAnnotation(
        visibleAnnotations: List<org.objectweb.asm.tree.AnnotationNode>?,
        invisibleAnnotations: List<org.objectweb.asm.tree.AnnotationNode>?,
    ): Boolean {
        if (config.excludeAnnotations.isEmpty()) return false
        val allAnnotations = (visibleAnnotations.orEmpty() + invisibleAnnotations.orEmpty())
        return allAnnotations.any { annotation ->
            val annotationName = annotation.desc
                ?.removePrefix("L")
                ?.removeSuffix(";")
                ?.replace('/', '.')
                ?: return@any false
            config.excludeAnnotations.any { pattern -> patternMatches(pattern, annotationName) }
        }
    }

}
