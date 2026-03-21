package com.jkjamies.omnivore.agent.android

import com.jkjamies.omnivore.agent.AgentConfig
import com.jkjamies.omnivore.agent.OmnivoreAgent
import com.jkjamies.omnivore.agent.runtime.ExecutionDataWriter
import com.jkjamies.omnivore.agent.runtime.ProbeMapWriter
import java.io.ByteArrayOutputStream
import java.io.File
import java.util.Base64

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
 * - Application classes are instrumented at build time by AGP transforms.
 * - OmnivoreRuntime.getProbes() is called from the instrumented <clinit>,
 *   which routes to OmnivoreAgent.dataStore.
 * - This listener ensures the agent is initialized before any instrumented
 *   class is loaded, and that data is flushed after tests complete.
 *
 * Coverage data extraction: Since Android's SELinux policy prevents app processes
 * from writing to /data/local/tmp/ (shell_data_file context), and AGP uninstalls
 * the app after tests (making run-as and internal storage inaccessible), we output
 * coverage data as base64 via System.err (logcat). The Gradle pull task reads it
 * back from `adb logcat` and decodes it. The data is typically under 1KB so this
 * approach is reliable and avoids all filesystem permission issues.
 */
class OmnivoreTestListener : org.junit.runner.notification.RunListener() {

    companion object {
        /** Markers used by the Gradle pull task to find coverage data in logcat output. */
        const val MARKER_EXEC_START = "OMNIVORE_EXEC_DATA_START"
        const val MARKER_EXEC_END = "OMNIVORE_EXEC_DATA_END"
        const val MARKER_PROBE_START = "OMNIVORE_PROBE_DATA_START"
        const val MARKER_PROBE_END = "OMNIVORE_PROBE_DATA_END"
    }

    private var destDir: File = File("/data/local/tmp/omnivore")

    override fun testRunStarted(description: org.junit.runner.Description?) {
        try {
            System.err.println("[Omnivore] TestListener.testRunStarted — initializing agent")
            val config = buildConfig()
            OmnivoreAgent.initialize(config)
            System.err.println("[Omnivore] Agent initialized. destDir=$destDir")
        } catch (e: Exception) {
            System.err.println("[Omnivore] Failed to initialize agent: ${e.message}")
            e.printStackTrace(System.err)
        }
    }

    override fun testRunFinished(result: org.junit.runner.Result?) {
        try {
            System.err.println("[Omnivore] TestListener.testRunFinished — flushing coverage data")
            flushCoverageData()
        } catch (e: Exception) {
            System.err.println("[Omnivore] Failed to flush coverage data: ${e.message}")
            e.printStackTrace(System.err)
        }
    }

    private fun buildConfig(): AgentConfig {
        val args = getInstrumentationArgs()

        destDir = resolveDestDir(args)
        System.err.println("[Omnivore] destDir=${destDir.absolutePath}, exists=${destDir.exists()}, writable=${destDir.canWrite()}")

        val destFile = File(destDir, "coverage.omnivore")
        val composeEnabled = (args["omnivore.compose"] ?: "true").toBooleanStrictOrNull() ?: true
        val includes = (args["omnivore.includes"] ?: "")
            .split(":").filter { it.isNotBlank() }
        val excludes = (args["omnivore.excludes"] ?: "")
            .split(":").filter { it.isNotBlank() }

        return AgentConfig(
            destFile = destFile,
            includes = includes,
            excludes = excludes,
            composeFilterEnabled = composeEnabled,
        )
    }

    /**
     * Read instrumentation arguments via AndroidX InstrumentationRegistry.
     */
    private fun getInstrumentationArgs(): Map<String, String> {
        return try {
            val registryClass = Class.forName("androidx.test.platform.app.InstrumentationRegistry")
            val getArgsMethod = registryClass.getMethod("getArguments")
            val bundle = getArgsMethod.invoke(null)

            val keySetMethod = bundle.javaClass.getMethod("keySet")
            @Suppress("UNCHECKED_CAST")
            val keys = keySetMethod.invoke(bundle) as Set<String>

            val getStringMethod = bundle.javaClass.getMethod("getString", String::class.java)
            keys.mapNotNull { key ->
                val value = getStringMethod.invoke(bundle, key) as? String
                if (value != null) key to value else null
            }.toMap()
        } catch (e: Exception) {
            System.err.println("[Omnivore] Could not read InstrumentationRegistry args: ${e.message}")
            emptyMap()
        }
    }

    /**
     * Resolve a writable directory for coverage data.
     * Uses the app's internal files directory which is always writable.
     */
    private fun resolveDestDir(args: Map<String, String>): File {
        try {
            val registryClass = Class.forName("androidx.test.platform.app.InstrumentationRegistry")
            val getInstMethod = registryClass.getMethod("getInstrumentation")
            val instrumentation = getInstMethod.invoke(null)

            val getContextMethod = instrumentation.javaClass.getMethod("getTargetContext")
            val context = getContextMethod.invoke(instrumentation)

            val getFilesDirMethod = context.javaClass.getMethod("getFilesDir")
            val filesDir = getFilesDirMethod.invoke(context) as? File
            if (filesDir != null) {
                val omnivoreDir = File(filesDir, "omnivore")
                omnivoreDir.mkdirs()
                if (omnivoreDir.canWrite()) return omnivoreDir
            }
        } catch (e: Exception) {
            System.err.println("[Omnivore] Could not get app files dir: ${e.message}")
        }

        return File(args["omnivore.destdir"] ?: "/data/local/tmp/omnivore")
    }

    private fun flushCoverageData() {
        try {
            val dataStore = OmnivoreAgent.getExecutionData()
            if (dataStore.isEmpty()) {
                System.err.println("[Omnivore] No execution data collected — dataStore is empty")
                return
            }

            // Write to internal storage (for local debugging)
            destDir.mkdirs()
            val destFile = File(destDir, "coverage.omnivore")
            ExecutionDataWriter.write(destFile, dataStore)
            System.err.println("[Omnivore] Wrote execution data: ${destFile.absolutePath} (${destFile.length()} bytes)")

            val probeMapFile = File(destDir, "coverage.probes")
            ProbeMapWriter.write(probeMapFile, OmnivoreAgent.probeMap)
            System.err.println("[Omnivore] Wrote probe map: ${probeMapFile.absolutePath} (${probeMapFile.length()} bytes)")

            // Output coverage data as base64 via logcat so Gradle can extract it.
            // This bypasses all filesystem/SELinux issues and survives app uninstallation.
            outputViaLogcat(destFile, MARKER_EXEC_START, MARKER_EXEC_END)
            outputViaLogcat(probeMapFile, MARKER_PROBE_START, MARKER_PROBE_END)

            val allData = dataStore.getAllData()
            val hitCount = allData.sumOf { d -> d.probes.count { it } }
            val totalCount = allData.sumOf { it.probes.size }
            System.err.println("[Omnivore] Coverage summary: $hitCount/$totalCount probes hit across ${allData.size} classes")
        } catch (e: Exception) {
            System.err.println("[Omnivore] Failed to write coverage data: ${e.message}")
            e.printStackTrace(System.err)
        }
    }

    /**
     * Output a file's contents as base64 via System.err (logcat).
     * The Gradle pull task extracts data between the start/end markers.
     */
    private fun outputViaLogcat(file: File, startMarker: String, endMarker: String) {
        try {
            val bytes = file.readBytes()
            val encoded = Base64.getEncoder().encodeToString(bytes)
            System.err.println(startMarker)
            System.err.println(encoded)
            System.err.println(endMarker)
        } catch (e: Exception) {
            System.err.println("[Omnivore] Failed to output ${file.name} via logcat: ${e.message}")
        }
    }
}
