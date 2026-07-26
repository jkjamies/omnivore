package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.runtime.ClassId
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.ClassNode

/**
 * Buffers a class into an ASM [ClassNode], then instruments it with the same
 * [ClassInstrumenter] core the JVM agent uses.
 *
 * This is the build-time (Android/AGP) entry point. AGP has no `-javaagent`, so
 * classes are instrumented during the build by an `AsmClassVisitorFactory`,
 * which hands the transform a *streaming* [ClassVisitor] rather than the class
 * bytes.
 *
 * ## Why buffer instead of instrumenting as we go
 *
 * Probe-array sizing needs a number that is not known until every method has
 * been seen. The previous implementation instrumented in a single streaming pass
 * and worked around that by emitting a hardcoded `getProbes(classId, name, 1024)`
 * from `<clinit>` — "generous pre-allocation". Two things were wrong with it:
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
 *
 * ## Why this lives in the agent module
 *
 * It has no AGP types in its signature, and the module that does (the Gradle
 * plugin) is `compileOnly` against AGP — which means its tests cannot run
 * without a full Android toolchain. Keeping the logic here lets
 * `omnivore-agent-tests` exercise the build-time path directly, and check it
 * against the load-time path on the same input. See
 * `BuildTimeInstrumentationTest`.
 */
class BuildTimeInstrumentingVisitor(
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
