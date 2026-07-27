package com.jkjamies.omnivore.agent.runtime

/**
 * Derives the stable identifier used to correlate a class's probe array
 * (`.omnivore`) with its probe map (`.probes`).
 *
 * ## Why the name, and not the bytecode
 *
 * JaCoCo identifies classes by a CRC64 of the class *file contents*, which has
 * the pleasant property that a recompiled class is self-evidently a different
 * class. Omnivore cannot do that: the ID is baked into the instrumented
 * `<clinit>` as an `LDC`, and the Android path instruments through AGP's
 * `AsmClassVisitorFactory`, which hands the transform a visitor rather than the
 * original bytes. There is nothing to hash there. Both paths must agree, so the
 * ID is derived from the class name.
 *
 * ## Why CRC64 and not the old hash
 *
 * The previous scheme was `hash = hash * 31 + char` accumulated into a `Long`.
 * Because the multiplier is small and the accumulator wide, short names never
 * mix into the high bits at all, and names sharing a long common prefix stay
 * numerically adjacent — exactly the shape of a real package hierarchy. A
 * collision silently merges two classes' coverage, with no error anywhere.
 *
 * CRC64 (ECMA-182) spreads a name change across the whole word, so collisions
 * over any realistic project are not a practical concern.
 *
 * Staleness is handled separately, by comparing probe counts at merge time —
 * see `OmnivoreReportTask.mergeData`.
 */
object ClassId {

    /** Compute the class identifier for an internal class name (`com/example/Foo`). */
    @JvmStatic
    fun forClassName(className: String): Long = crc64(className.toByteArray(Charsets.UTF_8))

    private val TABLE: LongArray = buildTable()

    /** CRC-64/ECMA-182, reflected form — the same polynomial JaCoCo uses. */
    private fun crc64(bytes: ByteArray): Long {
        var crc = 0L
        for (b in bytes) {
            val index = ((crc xor b.toLong()) and 0xFF).toInt()
            crc = TABLE[index] xor (crc ushr 8)
        }
        return crc
    }

    private fun buildTable(): LongArray {
        // Reflected polynomial for ECMA-182.
        val poly = -0x3693a86a2878f0beL // 0xC96C5795D7870F42
        val table = LongArray(256)
        for (i in 0 until 256) {
            var value = i.toLong()
            repeat(8) {
                value = if (value and 1L == 1L) (value ushr 1) xor poly else value ushr 1
            }
            table[i] = value
        }
        return table
    }
}
