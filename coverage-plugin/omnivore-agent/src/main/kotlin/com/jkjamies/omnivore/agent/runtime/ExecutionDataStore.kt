package com.jkjamies.omnivore.agent.runtime

import java.util.concurrent.ConcurrentHashMap

/**
 * Thread-safe store for coverage execution data.
 *
 * Each instrumented class gets a boolean probe array. When a probe point is hit
 * during execution, the corresponding array element is set to true.
 */
class ExecutionDataStore {

    /**
     * Maps class ID (based on class name hash) to its probe array.
     * The probe array is shared with the instrumented class — when the class
     * executes, it directly sets elements in this array.
     */
    private val probes = ConcurrentHashMap<Long, ProbeData>()

    /**
     * Register a class and get its probe array.
     * Called during instrumentation to create the probe storage.
     */
    fun getOrCreateProbes(classId: Long, className: String, probeCount: Int): BooleanArray {
        val data = probes.computeIfAbsent(classId) {
            ProbeData(
                classId = classId,
                className = className,
                probes = BooleanArray(probeCount)
            )
        }
        return data.probes
    }

    /**
     * Get all collected execution data.
     */
    fun getAllData(): Collection<ProbeData> = probes.values.toList()

    /**
     * Check if any data has been collected.
     */
    fun isEmpty(): Boolean = probes.isEmpty()

    /**
     * Reset all probe data.
     */
    fun reset() {
        probes.clear()
    }

    /**
     * Get data for a specific class.
     */
    fun getData(classId: Long): ProbeData? = probes[classId]
}

/**
 * Probe data for a single instrumented class.
 */
data class ProbeData(
    val classId: Long,
    val className: String,
    val probes: BooleanArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ProbeData) return false
        return classId == other.classId
    }

    override fun hashCode(): Int = classId.hashCode()
}
