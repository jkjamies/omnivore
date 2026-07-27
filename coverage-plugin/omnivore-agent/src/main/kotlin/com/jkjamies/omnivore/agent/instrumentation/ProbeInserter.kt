package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

/**
 * Inserts coverage probes into method bytecode.
 *
 * A "probe" is a single instruction sequence that sets a boolean array element
 * to true. The probe array is stored as a static field ($omnivoreProbes) in the
 * instrumented class and shared with the ExecutionDataStore via OmnivoreRuntime.
 *
 * ## Probe placement
 *
 * **Line probes** are inserted at each new line number, giving line coverage.
 *
 * **Branch probes are placed on control-flow *edges*, not on the branch
 * instruction.** This distinction is the whole point. A probe sitting in front
 * of a conditional jump fires when the condition is *evaluated*, which says
 * nothing about which way control went — `if (x) a() else b()` would report
 * full branch coverage from a test that only ever takes the `true` path. That
 * is wrong in the optimistic direction, which is the worst direction for a
 * quality gate: it silently inflates ratchet floors and PR comments.
 *
 * So each conditional jump is rewritten to route both outcomes through their
 * own probe block:
 *
 * ```text
 *     <operands>                       <operands>
 *     IFEQ target        becomes       IFEQ probeTaken
 *     <fall-through>                   probes[notTaken] = true
 *                                      GOTO after
 *                                  probeTaken:
 *                                      probes[taken] = true
 *                                      GOTO target
 *                                  after:
 *                                      <fall-through>
 * ```
 *
 * `TABLESWITCH` / `LOOKUPSWITCH` get the same treatment, one probe per case arm
 * plus one for `default` — previously they produced no probes at all, so `when`
 * over an enum or sealed class contributed nothing to branch coverage.
 *
 * Two probes per conditional means the probe indices produced here must be
 * mirrored *exactly* by `countProbes` and `buildProbeMap` in
 * `OmnivoreClassTransformer` (and by the AGP build-time visitor). If those
 * three disagree by even one index, coverage is silently misattributed to the
 * wrong lines. [BRANCH_PROBES_PER_JUMP] and [switchProbeCount] exist so the
 * arithmetic lives in one place.
 *
 * Each probe uses a global index offset so that multiple methods in the same
 * class share a single contiguous probe array.
 */
class ProbeInserter(
    private val probeArrayFieldOwner: String,
    private val globalOffset: Int,
    private val methodName: String,
    private val methodDesc: String,
    private val classProbeMap: ClassProbeMap?,
    delegate: MethodVisitor,
    var isComposable: Boolean = false,
) : MethodVisitor(Opcodes.ASM9, delegate) {

    private var localProbeCount = 0
    private var currentLine = -1
    private val seenLines = mutableSetOf<Int>()

    /**
     * Identifies the decision point a branch probe belongs to, so the reporter
     * can tell "1 of 2 edges of one `if`" from "2 separate branches". Counts up
     * per method; line probes use [NO_BRANCH].
     */
    private var branchGroup = 0

    /** Total probes inserted by this method visitor */
    val probeCount: Int get() = localProbeCount

    override fun visitLineNumber(line: Int, start: Label?) {
        super.visitLineNumber(line, start)
        currentLine = line
        if (seenLines.add(line)) {
            recordAndEmit(ProbeType.LINE, NO_BRANCH)
        }
    }

    override fun visitJumpInsn(opcode: Int, label: Label?) {
        // GOTO and JSR are unconditional: no decision, nothing to measure.
        if (opcode == Opcodes.GOTO || opcode == Opcodes.JSR || label == null) {
            super.visitJumpInsn(opcode, label)
            return
        }

        val group = branchGroup++
        val probeTaken = Label()
        val after = Label()

        // Redirect the "condition met" edge into its own probe block. The jump
        // has already consumed its operands, so the stack is empty at both the
        // fall-through and the probe block — COMPUTE_FRAMES handles the rest.
        super.visitJumpInsn(opcode, probeTaken)

        // Fall-through edge: condition not met.
        recordAndEmit(ProbeType.BRANCH, group)
        super.visitJumpInsn(Opcodes.GOTO, after)

        // Taken edge, then on to the original target.
        super.visitLabel(probeTaken)
        recordAndEmit(ProbeType.BRANCH, group)
        super.visitJumpInsn(Opcodes.GOTO, label)

        super.visitLabel(after)
    }

    override fun visitTableSwitchInsn(min: Int, max: Int, dflt: Label?, vararg labels: Label?) {
        val group = branchGroup++
        val defaultProbe = Label()
        val caseProbes = Array(labels.size) { Label() }

        super.visitTableSwitchInsn(min, max, defaultProbe, *caseProbes)

        emitSwitchProbeBlock(defaultProbe, dflt, group)
        for (i in labels.indices) {
            emitSwitchProbeBlock(caseProbes[i], labels[i], group)
        }
    }

    override fun visitLookupSwitchInsn(dflt: Label?, keys: IntArray?, labels: Array<out Label>?) {
        val targets = labels ?: emptyArray()
        val group = branchGroup++
        val defaultProbe = Label()
        val caseProbes = Array(targets.size) { Label() }

        super.visitLookupSwitchInsn(defaultProbe, keys, caseProbes)

        emitSwitchProbeBlock(defaultProbe, dflt, group)
        for (i in targets.indices) {
            emitSwitchProbeBlock(caseProbes[i], targets[i], group)
        }
    }

    /**
     * Emit `probeLabel: probes[i] = true; GOTO target`.
     *
     * Note that two case arms sharing a target (`case 1: case 2:`) still get a
     * probe each — they are distinct edges, and that is how JaCoCo counts them.
     */
    private fun emitSwitchProbeBlock(probeLabel: Label, target: Label?, group: Int) {
        super.visitLabel(probeLabel)
        recordAndEmit(ProbeType.BRANCH, group)
        if (target != null) {
            super.visitJumpInsn(Opcodes.GOTO, target)
        }
    }

    private fun recordAndEmit(type: ProbeType, branchGroup: Int) {
        val probeIndex = globalOffset + localProbeCount
        localProbeCount++

        // Record the mapping for report generation
        classProbeMap?.addProbe(
            probeIndex, currentLine, methodName, methodDesc, type, isComposable, branchGroup
        )

        emitProbeStore(probeIndex)
    }

    private fun emitProbeStore(probeIndex: Int) {
        mv.visitFieldInsn(
            Opcodes.GETSTATIC,
            probeArrayFieldOwner,
            PROBE_FIELD_NAME,
            PROBE_FIELD_DESCRIPTOR
        )
        emitIntPush(probeIndex)
        mv.visitInsn(Opcodes.ICONST_1)
        mv.visitInsn(Opcodes.BASTORE)
    }

    /** Emit the most efficient int-push instruction for the given value */
    private fun emitIntPush(value: Int) {
        when {
            value in -1..5 -> mv.visitInsn(Opcodes.ICONST_0 + value)
            value in Byte.MIN_VALUE..Byte.MAX_VALUE -> mv.visitIntInsn(Opcodes.BIPUSH, value)
            value in Short.MIN_VALUE..Short.MAX_VALUE -> mv.visitIntInsn(Opcodes.SIPUSH, value)
            else -> mv.visitLdcInsn(value)
        }
    }

    override fun visitMaxs(maxStack: Int, maxLocals: Int) {
        // Probe insertion adds GETSTATIC + index + ICONST_1 + BASTORE = 3 extra stack slots.
        // Bump max stack to accommodate probes inserted at any point in the method.
        // (COMPUTE_FRAMES recomputes this anyway; the bump keeps the visitor
        // correct if it is ever used with a writer that does not.)
        val adjustedMaxStack = if (localProbeCount > 0) maxStack + 3 else maxStack
        super.visitMaxs(adjustedMaxStack, maxLocals)
    }

    companion object {
        const val PROBE_FIELD_NAME = "\$omnivoreProbes"
        const val PROBE_FIELD_DESCRIPTOR = "[Z"

        /** Branch group value used for probes that are not part of a decision. */
        const val NO_BRANCH = -1

        /**
         * Probes emitted per conditional jump: one for the taken edge, one for
         * the fall-through. Counting code must use this rather than assuming.
         */
        const val BRANCH_PROBES_PER_JUMP = 2

        /** Probes emitted for a switch with [caseCount] arms: one per arm, plus `default`. */
        fun switchProbeCount(caseCount: Int): Int = caseCount + 1

        /** True for jump opcodes that represent a real two-way decision. */
        fun isConditionalJump(opcode: Int): Boolean =
            opcode != Opcodes.GOTO && opcode != Opcodes.JSR
    }
}
