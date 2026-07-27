package com.jkjamies.omnivore.gradle.transform

import com.android.build.api.instrumentation.AsmClassVisitorFactory
import com.android.build.api.instrumentation.ClassContext
import com.android.build.api.instrumentation.ClassData
import com.android.build.api.instrumentation.InstrumentationParameters
import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.instrumentation.BuildTimeInstrumentingVisitor
import com.jkjamies.omnivore.agent.instrumentation.ComposeDetector
import com.jkjamies.omnivore.agent.instrumentation.GlobPattern
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Optional
import org.objectweb.asm.ClassVisitor

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

        return BuildTimeInstrumentingVisitor(
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
