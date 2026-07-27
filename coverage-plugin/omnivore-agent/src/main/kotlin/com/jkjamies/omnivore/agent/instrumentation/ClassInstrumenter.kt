package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeEntry
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes
import org.objectweb.asm.tree.AbstractInsnNode
import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.LineNumberNode
import org.objectweb.asm.tree.LookupSwitchInsnNode
import org.objectweb.asm.tree.MethodNode
import org.objectweb.asm.tree.TableSwitchInsnNode

/**
 * The instrumentation core shared by both code paths.
 *
 * Omnivore instruments classes in two places: the JVM agent
 * ([OmnivoreClassTransformer], load-time, for unit tests) and the AGP build-time
 * transform (`OmnivoreClassVisitorFactory`, because Android has no `-javaagent`).
 * Those two used to carry independent copies of the probe-counting and
 * bytecode-emitting logic, and they had drifted: the AGP copy did not handle
 * switch statements, and — because AGP hands the transform a streaming visitor
 * rather than the class bytes — it could not know the real probe count when it
 * had to emit `<clinit>`, so it allocated a hardcoded 1024-element array for any
 * class that already had a static initialiser. A class with more than 1024
 * probes would have thrown `ArrayIndexOutOfBoundsException` inside the app under
 * test.
 *
 * Everything now runs through here, off an ASM [ClassNode], so both paths make
 * exactly the same decisions and allocate exactly the right array.
 *
 * ## The invariant that matters
 *
 * Probe indices are positional: the Nth probe emitted by [ProbeInserter] must be
 * the Nth probe described by [buildProbeMap]. [walkProbes] is the single
 * traversal both counting and mapping use, and it must stay in lockstep with
 * [ProbeInserter]'s emission order. Any divergence silently attributes coverage
 * to the wrong source lines — the kind of bug that produces plausible numbers
 * and no error at all.
 */
object ClassInstrumenter {

    /** Count the probes instrumentation will emit for [classNode]. */
    fun countProbes(classNode: ClassNode, config: AgentConfig): Int {
        var total = 0
        walkProbes(classNode, config) { _, _, _, _, _ -> total++ }
        return total
    }

    /** Populate [classProbeMap] with an entry per probe, in emission order. */
    fun buildProbeMap(classNode: ClassNode, config: AgentConfig, classProbeMap: ClassProbeMap) {
        var probeIndex = 0
        walkProbes(classNode, config) { method, isComposable, type, line, branchGroup ->
            classProbeMap.addProbe(
                probeIndex++,
                line,
                method.name ?: "",
                method.desc ?: "",
                type,
                isComposable,
                branchGroup,
            )
        }
    }

    /**
     * Walk [classNode] exactly as [ProbeInserter] would, invoking [onProbe] once
     * per probe that will be emitted, in order.
     */
    inline fun walkProbes(
        classNode: ClassNode,
        config: AgentConfig,
        onProbe: (method: MethodNode, isComposable: Boolean, type: ProbeType, line: Int, branchGroup: Int) -> Unit,
    ) {
        for (method in classNode.methods ?: emptyList()) {
            if (!isInstrumentableMethod(method, config)) continue

            val isComposable = ComposeDetector.isComposableMethod(method)
            var currentLine = -1
            var branchGroup = 0
            val seenLines = mutableSetOf<Int>()

            for (insn in method.instructions ?: continue) {
                when (insn.type) {
                    AbstractInsnNode.LINE -> {
                        val line = (insn as LineNumberNode).line
                        currentLine = line
                        if (seenLines.add(line)) {
                            onProbe(method, isComposable, ProbeType.LINE, line, ProbeEntry.NO_BRANCH)
                        }
                    }

                    AbstractInsnNode.JUMP_INSN -> {
                        if (ProbeInserter.isConditionalJump(insn.opcode)) {
                            val group = branchGroup++
                            repeat(ProbeInserter.BRANCH_PROBES_PER_JUMP) {
                                onProbe(method, isComposable, ProbeType.BRANCH, currentLine, group)
                            }
                        }
                    }

                    AbstractInsnNode.TABLESWITCH_INSN -> {
                        val group = branchGroup++
                        val count = ProbeInserter.switchProbeCount(
                            (insn as TableSwitchInsnNode).labels?.size ?: 0
                        )
                        repeat(count) {
                            onProbe(method, isComposable, ProbeType.BRANCH, currentLine, group)
                        }
                    }

                    AbstractInsnNode.LOOKUPSWITCH_INSN -> {
                        val group = branchGroup++
                        val count = ProbeInserter.switchProbeCount(
                            (insn as LookupSwitchInsnNode).labels?.size ?: 0
                        )
                        repeat(count) {
                            onProbe(method, isComposable, ProbeType.BRANCH, currentLine, group)
                        }
                    }
                }
            }
        }
    }

    /**
     * The method filter shared by counting, mapping, and instrumentation.
     *
     * `<clinit>` is excluded because [InstrumentingClassVisitor] handles it
     * separately — it prepends the probe-array initialisation and inserts no
     * probes of its own.
     */
    fun isInstrumentableMethod(method: MethodNode, config: AgentConfig): Boolean {
        val name = method.name ?: return false
        if (name == "<clinit>") return false
        val access = method.access
        if ((access and Opcodes.ACC_BRIDGE) != 0) return false
        if ((access and Opcodes.ACC_ABSTRACT) != 0) return false
        if ((access and Opcodes.ACC_NATIVE) != 0) return false
        if (KotlinDetector.isSyntheticBridgeMethod(method)) return false
        if (config.composeFilterEnabled && ComposeDetector.isComposeLambdaGroup(name)) return false
        return true
    }
}

/**
 * ASM ClassVisitor that instruments methods with coverage probes and generates
 * the probe initialization code in `<clinit>`.
 *
 * [totalProbeCount] must be the value [ClassInstrumenter.countProbes] returned
 * for the same [classNode]: it sizes the probe array that every probe store
 * writes into.
 */
class InstrumentingClassVisitor(
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
    private var isInterface = false

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
            if (isInterface || totalProbeCount == 0) return mv
            return ClinitPrefixVisitor(classId, className, totalProbeCount, mv)
        }

        val mv = super.visitMethod(access, name, descriptor, signature, exceptions) ?: return null
        if (isInterface || totalProbeCount == 0) return mv

        val methodNode = classNode.methods?.find { it.name == name && it.desc == descriptor }
            ?: return mv
        if (!ClassInstrumenter.isInstrumentableMethod(methodNode, config)) return mv

        val currentOffset = globalProbeOffset
        val probeInserter = ProbeInserter(
            probeArrayFieldOwner = className,
            globalOffset = currentOffset,
            methodName = name,
            methodDesc = descriptor,
            classProbeMap = classProbeMap,
            delegate = mv,
            isComposable = ComposeDetector.isComposableMethod(methodNode),
        )
        return ProbeCountingMethodVisitor(probeInserter) { count ->
            globalProbeOffset += count
        }
    }

    override fun visitEnd() {
        // Interfaces cannot carry an ACC_TRANSIENT static field, and a class
        // with no probes needs no array.
        if (!isInterface && totalProbeCount > 0) {
            super.visitField(
                Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC or Opcodes.ACC_SYNTHETIC or Opcodes.ACC_TRANSIENT,
                ProbeInserter.PROBE_FIELD_NAME,
                ProbeInserter.PROBE_FIELD_DESCRIPTOR,
                null,
                null
            )?.visitEnd()

            if (!hasExistingClinit) {
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
}

/** Prepends probe initialization to an existing `<clinit>`. */
internal class ClinitPrefixVisitor(
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
internal class ProbeCountingMethodVisitor(
    private val probeInserter: ProbeInserter,
    private val onEnd: (Int) -> Unit,
) : MethodVisitor(Opcodes.ASM9, probeInserter) {
    override fun visitEnd() {
        super.visitEnd()
        onEnd(probeInserter.probeCount)
    }
}

/**
 * Emit `$omnivoreProbes = OmnivoreRuntime.getProbes(classId, className, probeCount)`.
 */
internal fun emitProbeInit(mv: MethodVisitor, classId: Long, className: String, probeCount: Int) {
    mv.visitLdcInsn(classId)
    mv.visitLdcInsn(className.replace('/', '.'))
    emitProbeInitIntPush(mv, probeCount)
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

internal fun emitProbeInitIntPush(mv: MethodVisitor, value: Int) {
    when {
        value in -1..5 -> mv.visitInsn(Opcodes.ICONST_0 + value)
        value in Byte.MIN_VALUE..Byte.MAX_VALUE -> mv.visitIntInsn(Opcodes.BIPUSH, value)
        value in Short.MIN_VALUE..Short.MAX_VALUE -> mv.visitIntInsn(Opcodes.SIPUSH, value)
        else -> mv.visitLdcInsn(value)
    }
}

/**
 * A [ClassWriter] that resolves type hierarchies through the classloader of the
 * class being transformed.
 *
 * `COMPUTE_FRAMES` has to compute a common supertype whenever two branches merge
 * with different reference types on the stack, and ASM's default
 * `getCommonSuperClass` does that with `Class.forName` on *its own* loader — the
 * agent loader, which typically cannot see application classes at all.
 *
 * Every such class threw `ClassNotFoundException` (wrapped in
 * `TypeNotPresentException`), which the transformer caught and turned into
 * "return the class uninstrumented". The failure was silent and precisely
 * inverted: simple classes instrumented fine, while the branch-heavy classes
 * that matter most for coverage were skipped, showing up as 0% or absent.
 *
 * Resolving against the target loader (falling back to the agent's own, then to
 * `java/lang/Object`) keeps frame computation working without ever failing the
 * transform. Edge-based branch probes add merge points, which makes getting this
 * right more load-bearing than before.
 */
class LoaderAwareClassWriter(
    private val loader: ClassLoader?,
) : ClassWriter(COMPUTE_FRAMES) {

    override fun getCommonSuperClass(type1: String, type2: String): String {
        return try {
            val effectiveLoader = loader ?: javaClass.classLoader
            val c = Class.forName(type1.replace('/', '.'), false, effectiveLoader)
            val d = Class.forName(type2.replace('/', '.'), false, effectiveLoader)

            when {
                c.isAssignableFrom(d) -> type1
                d.isAssignableFrom(c) -> type2
                c.isInterface || d.isInterface -> OBJECT
                else -> {
                    var candidate = c
                    while (!candidate.isAssignableFrom(d)) {
                        candidate = candidate.superclass ?: return OBJECT
                    }
                    candidate.name.replace('.', '/')
                }
            }
        } catch (_: Throwable) {
            // Object is always a safe answer: it widens the frame, which the
            // verifier accepts, where an exception would lose the class.
            OBJECT
        }
    }

    private companion object {
        const val OBJECT = "java/lang/Object"
    }
}
