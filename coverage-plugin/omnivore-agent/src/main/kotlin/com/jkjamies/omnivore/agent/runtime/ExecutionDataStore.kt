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
        val data = probes.compute(classId) { _, existing ->
            when {
                existing == null -> ProbeData(classId, className, BooleanArray(probeCount))

                // A second registration asking for more probes than the array
                // has means two differently-instrumented versions of the same
                // class are live: a stale class file on the classpath, or the
                // same class loaded by two classloaders. Class IDs are derived
                // from the name, so both land on this entry.
                //
                // Returning the smaller array (the old behaviour) meant the
                // newer class wrote past its end and threw
                // ArrayIndexOutOfBoundsException *inside the code under test* —
                // a coverage tool must never be able to crash the program it is
                // measuring. So grow instead, carrying existing hits across.
                //
                // Note the tradeoff this accepts: the earlier class already
                // holds a reference to the old array in its $omnivoreProbes
                // field, so its *subsequent* probe writes land somewhere no
                // longer reachable from the store and are lost. Coverage for one
                // of the two versions is unavoidably wrong here; losing some
                // data is strictly better than an exception, and the report task
                // warns about the count mismatch it will see downstream.
                existing.probes.size < probeCount -> {
                    val grown = BooleanArray(probeCount)
                    existing.probes.copyInto(grown)
                    ProbeData(classId, className, grown)
                }

                else -> existing
            }
        }
        return data!!.probes
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
