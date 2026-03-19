package com.jkjamies.omnivore.agent.instrumentation

import com.jkjamies.omnivore.agent.runtime.ClassProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.objectweb.asm.Label
import org.objectweb.asm.MethodVisitor
import org.objectweb.asm.Opcodes

/**
 * Inserts coverage probes into method bytecode.
 *
 * A "probe" is a simple instruction that sets a boolean array element to true.
 * The probe array is stored as a static field ($omnivoreProbes) in the
 * instrumented class and shared with the ExecutionDataStore via OmnivoreRuntime.
 *
 * Probes are inserted at:
 * - Each line number change (line coverage)
 * - Each conditional branch point (branch coverage)
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
) : MethodVisitor(Opcodes.ASM9, delegate) {

    private var localProbeCount = 0
    private var currentLine = -1
    private val seenLines = mutableSetOf<Int>()

    /** Total probes inserted by this method visitor */
    val probeCount: Int get() = localProbeCount

    override fun visitLineNumber(line: Int, start: Label?) {
        super.visitLineNumber(line, start)
        currentLine = line
        if (seenLines.add(line)) {
            insertProbe(ProbeType.LINE)
        }
    }

    override fun visitJumpInsn(opcode: Int, label: Label?) {
        if (opcode != Opcodes.GOTO) {
            insertProbe(ProbeType.BRANCH)
        }
        super.visitJumpInsn(opcode, label)
    }

    private fun insertProbe(type: ProbeType) {
        val probeIndex = globalOffset + localProbeCount
        localProbeCount++

        // Record the mapping for report generation
        classProbeMap?.addProbe(probeIndex, currentLine, methodName, methodDesc, type)

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
        val adjustedMaxStack = if (localProbeCount > 0) maxStack + 3 else maxStack
        super.visitMaxs(adjustedMaxStack, maxLocals)
    }

    companion object {
        const val PROBE_FIELD_NAME = "\$omnivoreProbes"
        const val PROBE_FIELD_DESCRIPTOR = "[Z"
    }
}
