package com.jkjamies.omnivore.agent.android

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.OmnivoreAgent
import com.jkjamies.omnivore.agent.runtime.ExecutionDataWriter
import com.jkjamies.omnivore.agent.runtime.ProbeMapWriter
import java.io.File

/**
 * JUnit test listener for Android instrumented tests.
 *
 * Implements the JUnit RunListener interface to:
 * 1. Initialize OmnivoreAgent at the start of the test run
 * 2. Flush coverage data to device storage when tests complete
 *
 * This class is referenced by name in the test instrumentation runner arguments
 * and loaded reflectively by the Android test runner. It must be on the test APK
 * classpath (added via androidTestImplementation).
 *
 * On Android, there is no -javaagent support. Instead:
 * - Application classes are instrumented at build time by AGP transforms or
 *   by the OmnivoreClassTransformer during the build pipeline.
 * - OmnivoreRuntime.getProbes() is called from the instrumented <clinit>,
 *   which routes to OmnivoreAgent.dataStore.
 * - This listener ensures the agent is initialized before any instrumented
 *   class is loaded, and that data is flushed after tests complete.
 *
 * Coverage data is written to /data/local/tmp/omnivore/ on the device,
 * then pulled by the omnivorePullCoverage Gradle task.
 */
class OmnivoreTestListener : org.junit.runner.notification.RunListener() {

    private var destDir: File = File("/data/local/tmp/omnivore")

    override fun testRunStarted(description: org.junit.runner.Description?) {
        // Read configuration from instrumentation arguments
        val config = buildConfig()
        OmnivoreAgent.initialize(config)
    }

    override fun testRunFinished(result: org.junit.runner.Result?) {
        flushCoverageData()
    }

    private fun buildConfig(): AgentConfig {
        // On Android, we read config from system properties set via
        // testInstrumentationRunnerArguments. The Android test runner
        // passes these as -e key value pairs which become available
        // via InstrumentationRegistry. However, since we're in a
        // RunListener (not an Instrumentation), we read from system props
        // that the bootstrap sets, or use defaults.
        val destDirPath = System.getProperty("omnivore.destdir", "/data/local/tmp/omnivore")
        destDir = File(destDirPath)
        destDir.mkdirs()

        val destFile = File(destDir, "coverage.omnivore")
        val composeEnabled = System.getProperty("omnivore.compose", "true").toBooleanStrictOrNull() ?: true
        val includes = System.getProperty("omnivore.includes", "")
            .split(":").filter { it.isNotBlank() }
        val excludes = System.getProperty("omnivore.excludes", "")
            .split(":").filter { it.isNotBlank() }

        return AgentConfig(
            destFile = destFile,
            includes = includes,
            excludes = excludes,
            composeFilterEnabled = composeEnabled,
        )
    }

    private fun flushCoverageData() {
        try {
            val dataStore = OmnivoreAgent.getExecutionData()
            if (dataStore.isEmpty()) {
                return
            }

            destDir.mkdirs()
            val destFile = File(destDir, "coverage.omnivore")
            ExecutionDataWriter.write(destFile, dataStore)

            val probeMapFile = File(destDir, "coverage.probes")
            ProbeMapWriter.write(probeMapFile, OmnivoreAgent.probeMap)
        } catch (e: Exception) {
            // Log but don't fail the test run
            System.err.println("Omnivore: Failed to write coverage data: ${e.message}")
        }
    }
}
