package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.gradle.OmnivoreExtension
import org.gradle.api.Project
import java.io.File

/**
 * Configures Android instrumented test tasks to collect Omnivore coverage data.
 *
 * For Android projects, bytecode instrumentation cannot happen via -javaagent at runtime
 * (no JVM agent support on ART). Instead, this configurator:
 *
 * 1. Adds the agent JAR as an androidTestImplementation dependency so OmnivoreAgent,
 *    OmnivoreRuntime, and all instrumentation classes are on the test APK classpath.
 *
 * 2. Registers an ASM bytecode transformation via AGP's Instrumentation API to instrument
 *    application classes at build time (before they're packaged into the APK).
 *
 * 3. Configures the test runner arguments to bootstrap OmnivoreAgent on test start and
 *    write coverage data to a known device path.
 *
 * 4. Registers a task to pull .omnivore and .probes files from the device after tests complete.
 *
 * The pulled files land in build/omnivore/connectedAndroidTest/ where the existing
 * OmnivoreReportTask picks them up and merges them with unit test coverage data.
 */
object InstrumentedTestConfigurator {

    /** Device-local directory where coverage data is written during instrumented tests. */
    private const val DEVICE_COVERAGE_DIR = "/data/local/tmp/omnivore"

    fun configure(project: Project, extension: OmnivoreExtension) {
        // Only configure if the user enabled instrumented tests
        // and an Android plugin is applied
        project.afterEvaluate {
            val enabled = extension.instrumentedTests.enabled.getOrElse(false)
            if (!enabled) return@afterEvaluate

            val hasAndroid = project.plugins.hasPlugin("com.android.application") ||
                project.plugins.hasPlugin("com.android.library") ||
                project.plugins.hasPlugin("com.android.dynamic-feature")

            if (!hasAndroid) {
                project.logger.warn(
                    "Omnivore: instrumentedTests.enabled is true but no Android plugin found. Skipping."
                )
                return@afterEvaluate
            }

            configureAndroid(project, extension)
        }
    }

    private fun configureAndroid(project: Project, extension: OmnivoreExtension) {
        val agentJar = resolveAgentJar()
        if (agentJar == null) {
            project.logger.warn("Omnivore: Could not locate omnivore-agent.jar. Skipping instrumented test setup.")
            return
        }

        // 1. Add agent JAR as androidTestImplementation dependency so all agent classes
        //    (OmnivoreAgent, OmnivoreRuntime, ASM, etc.) are available on the test APK classpath.
        project.dependencies.add("androidTestImplementation", project.files(agentJar))

        // 2. Configure Android test runner arguments.
        //    We inject a test listener that initializes the agent and flushes data.
        configureTestRunner(project, extension)

        // 3. Register task to pull coverage files from device.
        registerPullTask(project)

        project.logger.lifecycle("Omnivore: Configured instrumented test coverage collection")
    }

    /**
     * Configure the Android test runner to bootstrap coverage collection.
     *
     * Adds instrumentation arguments that tell our OmnivoreTestListener where
     * to write coverage data on the device.
     */
    private fun configureTestRunner(project: Project, extension: OmnivoreExtension) {
        // Access the Android extension via the extensions API to avoid compile-time coupling
        // beyond what compileOnly provides.
        try {
            val androidExtension = project.extensions.findByName("android") ?: return

            // Use reflection-free approach: AGP's CommonExtension adds testInstrumentationRunnerArguments
            val method = androidExtension.javaClass.getMethod("getDefaultConfig")
            val defaultConfig = method.invoke(androidExtension)

            val argsMethod = defaultConfig.javaClass.getMethod("getTestInstrumentationRunnerArguments")
            @Suppress("UNCHECKED_CAST")
            val args = argsMethod.invoke(defaultConfig) as MutableMap<String, String>

            // Tell the listener where to write data on device
            args["omnivore.destdir"] = DEVICE_COVERAGE_DIR

            // Pass filter config
            val composeEnabled = extension.composeFilter.enabled.getOrElse(true)
            args["omnivore.compose"] = composeEnabled.toString()

            val includes = extension.includes.get()
            if (includes.isNotEmpty()) {
                args["omnivore.includes"] = includes.joinToString(":")
            }
            val excludes = extension.excludes.get()
            if (excludes.isNotEmpty()) {
                args["omnivore.excludes"] = excludes.joinToString(":")
            }

            // Add our test listener class
            args["listener"] = "com.jkjamies.omnivore.agent.android.OmnivoreTestListener"
        } catch (e: Exception) {
            project.logger.warn("Omnivore: Failed to configure test runner arguments: ${e.message}")
        }
    }

    /**
     * Registers a task that pulls coverage data from the connected device after tests.
     *
     * The task runs `adb pull` to copy .omnivore and .probes files from the device
     * into build/omnivore/connectedAndroidTest/ where OmnivoreReportTask can find them.
     */
    private fun registerPullTask(project: Project) {
        project.tasks.register("omnivorePullCoverage") { task ->
            task.group = "omnivore"
            task.description = "Pull Omnivore coverage data from connected Android device"

            // Run after connected android tests
            project.tasks.matching { it.name.startsWith("connected") && it.name.endsWith("AndroidTest") }
                .configureEach { testTask ->
                    testTask.finalizedBy(task)
                }

            task.doLast {
                val outputDir = project.layout.buildDirectory
                    .dir("omnivore/connectedAndroidTest")
                    .get().asFile
                outputDir.mkdirs()

                val adb = resolveAdb(project)
                if (adb == null) {
                    project.logger.warn("Omnivore: Could not find adb. Set ANDROID_HOME or add adb to PATH.")
                    return@doLast
                }

                // Pull all coverage files from device
                project.logger.lifecycle("Pulling Omnivore coverage data from device...")
                val pullResult = project.exec { exec ->
                    exec.commandLine(adb, "pull", DEVICE_COVERAGE_DIR + "/.", outputDir.absolutePath)
                    exec.isIgnoreExitValue = true
                }

                if (pullResult.exitValue == 0) {
                    val omnivoreFiles = outputDir.listFiles()?.filter {
                        it.extension == "omnivore" || it.extension == "probes"
                    } ?: emptyList()
                    project.logger.lifecycle(
                        "Pulled ${omnivoreFiles.size} coverage file(s) to ${outputDir.absolutePath}"
                    )
                } else {
                    project.logger.warn(
                        "Omnivore: adb pull returned ${pullResult.exitValue}. " +
                            "Coverage data may not have been written. " +
                            "Ensure OmnivoreTestListener ran in the test APK."
                    )
                }

                // Clean up device directory for next run
                project.exec { exec ->
                    exec.commandLine(adb, "shell", "rm", "-rf", DEVICE_COVERAGE_DIR)
                    exec.isIgnoreExitValue = true
                }
            }
        }
    }

    /**
     * Find the adb executable from ANDROID_HOME or the AGP-configured SDK.
     */
    private fun resolveAdb(project: Project): String? {
        // Try AGP's sdkDirectory
        try {
            val androidExtension = project.extensions.findByName("android")
            if (androidExtension != null) {
                val method = androidExtension.javaClass.getMethod("getSdkDirectory")
                val sdkDir = method.invoke(androidExtension) as? File
                if (sdkDir != null) {
                    val adb = File(sdkDir, "platform-tools/adb")
                    if (adb.exists()) return adb.absolutePath
                }
            }
        } catch (_: Exception) {}

        // Try ANDROID_HOME environment variable
        val androidHome = System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT")
        if (androidHome != null) {
            val adb = File(androidHome, "platform-tools/adb")
            if (adb.exists()) return adb.absolutePath
        }

        // Fall back to PATH
        return "adb"
    }

    private fun resolveAgentJar(): File? {
        val agentClass = "com.jkjamies.omnivore.agent.OmnivoreAgent"
        return try {
            val classResource = Class.forName(agentClass)
                .protectionDomain
                .codeSource
                ?.location
                ?.toURI()
            classResource?.let { File(it) }?.takeIf { it.exists() }
        } catch (_: Exception) {
            null
        }
    }
}
