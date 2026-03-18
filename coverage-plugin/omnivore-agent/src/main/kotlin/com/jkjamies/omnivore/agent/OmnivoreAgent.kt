package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ShutdownHook
import java.lang.instrument.Instrumentation

/**
 * Omnivore Coverage Agent entry point.
 *
 * Operates in two modes:
 * - JVM Agent: Attached via -javaagent for unit tests
 * - Android Agent: Bootstrapped in instrumented test APK
 */
object OmnivoreAgent {

    private var initialized = false
    internal lateinit var dataStore: ExecutionDataStore
        private set
    internal lateinit var probeMap: ProbeMap
        private set
    internal lateinit var config: AgentConfig
        private set

    /**
     * JVM agent premain entry point.
     * Called by the JVM when -javaagent:omnivore-agent.jar is specified.
     */
    @JvmStatic
    fun premain(agentArgs: String?, instrumentation: Instrumentation) {
        initialize(AgentConfig.parse(agentArgs))
        instrumentation.addTransformer(
            OmnivoreClassTransformer(dataStore, probeMap, config),
            true
        )
    }

    /**
     * Initialize the agent with the given configuration.
     * Can be called directly for Android instrumented test mode.
     */
    @Synchronized
    fun initialize(agentConfig: AgentConfig = AgentConfig()) {
        if (initialized) return
        config = agentConfig
        dataStore = ExecutionDataStore()
        probeMap = ProbeMap()
        ShutdownHook.register(dataStore, probeMap, config)
        initialized = true
    }

    /**
     * Get the execution data store. Used by the reporter to read coverage data.
     */
    fun getExecutionData(): ExecutionDataStore {
        check(initialized) { "OmnivoreAgent has not been initialized" }
        return dataStore
    }
}
