package com.jkjamies.omnivore.agent.runtime

/**
 * Runtime helper called by instrumented classes to get their probe arrays.
 *
 * When an instrumented class is loaded, its generated <clinit> calls:
 *   OmnivoreRuntime.getProbes(classId, className, probeCount)
 *
 * This returns a boolean[] shared with the ExecutionDataStore, so when
 * probes are hit during execution, the data store sees the updates.
 *
 * Uses a lazy-init pattern (like JaCoCo's Offline class) so instrumented
 * classes can load before the agent is explicitly initialized. On Android,
 * app classes load before OmnivoreTestListener.testRunStarted() runs —
 * without lazy init, <clinit> would crash with UninitializedPropertyAccessException.
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
     * Default data store created lazily on first probe request.
     * This ensures instrumented classes can always load, even before
     * the agent is explicitly initialized. OmnivoreAgent.initialize()
     * adopts this store so all data ends up in the same place.
     */
    @JvmStatic
    val defaultDataStore: ExecutionDataStore by lazy { ExecutionDataStore() }

    /**
     * Override data store. When set, getProbes() uses this instead
     * of the default. Used by OmnivoreAgent to point at its own store,
     * or by tests to inject a mock store.
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
        val store = dataStoreOverride ?: defaultDataStore
        return store.getOrCreateProbes(classId, className, probeCount)
    }
}
