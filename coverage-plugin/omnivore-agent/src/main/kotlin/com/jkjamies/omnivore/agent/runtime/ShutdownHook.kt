package com.jkjamies.omnivore.agent.runtime

import com.jkjamies.omnivore.agent.AgentConfig
import java.io.File

/**
 * Registers a JVM shutdown hook to flush execution data and probe map
 * to disk when the test process exits.
 */
object ShutdownHook {

    private var registered = false

    @Synchronized
    fun register(dataStore: ExecutionDataStore, probeMap: ProbeMap, config: AgentConfig) {
        if (registered) return
        registered = true

        Runtime.getRuntime().addShutdownHook(Thread({
            if (!dataStore.isEmpty()) {
                ExecutionDataWriter.write(config.destFile, dataStore)

                // Write probe map alongside execution data
                val probeMapFile = File(
                    config.destFile.parentFile,
                    config.destFile.nameWithoutExtension + ".probes"
                )
                ProbeMapWriter.write(probeMapFile, probeMap)
            }
        }, "omnivore-shutdown-hook"))
    }
}
