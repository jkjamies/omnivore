package com.jkjamies.omnivore.agent.runtime

import com.jkjamies.omnivore.agent.OmnivoreAgent

/**
 * Runtime helper called by instrumented classes to get their probe arrays.
 *
 * When an instrumented class is loaded, its generated <clinit> calls:
 *   OmnivoreRuntime.getProbes(classId, className, probeCount)
 *
 * This returns a boolean[] shared with the ExecutionDataStore, so when
 * probes are hit during execution, the data store sees the updates.
 *
 * IMPORTANT: This class must remain stable — its method signatures are
 * baked into the bytecode of every instrumented class.
 */
object OmnivoreRuntime {

    /** Internal name used in bytecode generation */
    const val INTERNAL_NAME = "com/jkjamies/omnivore/agent/runtime/OmnivoreRuntime"

    /** Method name called from instrumented class <clinit> */
    const val GET_PROBES_METHOD = "getProbes"

    /** Method descriptor: (JLjava/lang/String;I)[Z */
    const val GET_PROBES_DESCRIPTOR = "(JLjava/lang/String;I)[Z"

    /**
     * Override data store for testing. When set, getProbes() uses this
     * instead of the global OmnivoreAgent.dataStore.
     */
    @Volatile
    @JvmStatic
    var dataStoreOverride: ExecutionDataStore? = null

    /**
     * Called from the generated <clinit> of instrumented classes.
     *
     * @param classId Stable hash of the class name
     * @param className Internal class name (e.g., "com/example/MyClass")
     * @param probeCount Number of probes inserted in this class
     * @return A boolean array shared with the ExecutionDataStore
     */
    @JvmStatic
    fun getProbes(classId: Long, className: String, probeCount: Int): BooleanArray {
        val store = dataStoreOverride ?: OmnivoreAgent.dataStore
        return store.getOrCreateProbes(classId, className, probeCount)
    }
}
