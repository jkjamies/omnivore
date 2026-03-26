package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.agent.runtime.ProbeMapWriter
import com.jkjamies.omnivore.gradle.OmnivoreExtension
import com.jkjamies.omnivore.gradle.transform.OmnivoreClassVisitorFactory
import org.gradle.api.Project
import java.io.File
import java.util.Properties

/**
 * Configures Android instrumented test tasks to collect Omnivore coverage data.
 *
 * Follows the JaCoCo pattern: instead of resolving JARs from the classloader's
 * protectionDomain (fragile in composite builds), we use Gradle's dependency
 * management to resolve the slim runtime artifact via a detached configuration.
 *
 * For Android projects, bytecode instrumentation cannot happen via -javaagent at runtime
 * (no JVM agent support on ART). Instead, this configurator:
 *
 * 1. Resolves the slim runtime JAR (Omnivore classes only) via Gradle configuration
 *    and adds it as an `implementation` dependency so OmnivoreRuntime is on the app
 *    APK classpath. The slim JAR avoids duplicate class conflicts with kotlin-stdlib,
 *    ASM, etc. bundled in the fat agent JAR.
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

    /** Resource path for the version properties generated at plugin build time. */
    private const val VERSION_PROPS_RESOURCE = "omnivore-version.properties"

    fun configure(project: Project, extension: OmnivoreExtension) {
        // Register AGP build-time transform eagerly (must happen during configuration phase,
        // not afterEvaluate). This instruments application classes with coverage probes before
        // they are dexed. The JVM agent detects already-instrumented classes during unit tests
        // and builds probe maps without re-instrumenting.
        OmnivoreTransformConfigurator.configure(project, extension)

        // Add runtime dependency eagerly when Android plugin is detected.
        // AGP resolves configurations during the configuration phase, so dependencies
        // added in afterEvaluate are silently ignored. The runtime JAR is tiny and
        // harmless even if instrumentedTests is disabled.
        addRuntimeDependencyEagerly(project)

        // Test runner args and task registration can wait for afterEvaluate
        // since they don't affect dependency resolution.
        val configureAction = Runnable {
            val enabled = extension.instrumentedTests.enabled.getOrElse(false)
            if (!enabled) return@Runnable

            val hasAndroid = project.plugins.hasPlugin("com.android.application") ||
                project.plugins.hasPlugin("com.android.library") ||
                project.plugins.hasPlugin("com.android.dynamic-feature")

            if (!hasAndroid) {
                project.logger.warn(
                    "Omnivore: instrumentedTests.enabled is true but no Android plugin found. Skipping."
                )
                return@Runnable
            }

            configureAndroidTestInfrastructure(project, extension)
        }

        if (project.state.executed) {
            configureAction.run()
        } else {
            project.afterEvaluate { configureAction.run() }
        }
    }

    /**
     * Add the runtime dependency during configuration phase, before AGP resolves
     * configurations. Reacts to Android plugin application to trigger at the right time.
     */
    private fun addRuntimeDependencyEagerly(project: Project) {
        val androidPluginIds = listOf(
            "com.android.application",
            "com.android.library",
            "com.android.dynamic-feature"
        )
        for (pluginId in androidPluginIds) {
            project.plugins.withId(pluginId) {
                val runtimeJar = resolveRuntimeJar(project)
                if (runtimeJar != null) {
                    project.dependencies.add("implementation", project.files(runtimeJar))
                    project.logger.info("Omnivore: Added runtime dependency to ${project.path}: ${runtimeJar.absolutePath}")
                } else {
                    project.logger.warn("Omnivore: Could not resolve omnivore-agent runtime JAR")
                }
            }
        }
    }

    /**
     * Configure test runner args, probe map task, and pull task.
     * Called from afterEvaluate since these don't affect dependency resolution.
     */
    private fun configureAndroidTestInfrastructure(project: Project, extension: OmnivoreExtension) {
        // 1. Configure Android test runner arguments.
        configureTestRunner(project, extension)

        // 2. Register task to write the build-time probe map (accumulated during AGP transform).
        registerProbeMapTask(project)

        // 3. Register setup task that creates a writable directory on device before tests run.
        registerSetupTask(project)

        // 4. Register task to pull coverage files from device.
        registerPullTask(project)

        project.logger.info("Omnivore: Configured instrumented test coverage collection")
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

            // Compose filter is always enabled (zero-cost on non-Compose projects)
            args["omnivore.compose"] = "true"

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

            project.logger.info("Omnivore: Set test runner args: $args")
        } catch (e: Exception) {
            project.logger.warn("Omnivore: Failed to configure test runner arguments: ${e.message}")
        }
    }

    /**
     * Registers a task that writes the build-time probe map accumulated during the
     * AGP bytecode transform. This probe map is needed for the report task to
     * correlate instrumented test probe data to source lines.
     *
     * The task runs after the ASM transform and before the report task.
     */
    private fun registerProbeMapTask(project: Project) {
        val probeMapTask = project.tasks.register("omnivoreWriteBuildProbeMap") { task ->
            task.group = "omnivore"
            task.description = "Write probe map from build-time bytecode transformation"

            task.doLast {
                val probeMap = OmnivoreClassVisitorFactory.buildTimeProbeMap
                if (probeMap.getAllClassMaps().isEmpty()) {
                    project.logger.info("Omnivore: No probe map data from build-time transform")
                    return@doLast
                }

                val outputDir = project.rootProject.layout.buildDirectory
                    .dir("omnivore/connectedAndroidTest")
                    .get().asFile
                outputDir.mkdirs()

                val probeFile = File(outputDir, "build-time.probes")
                ProbeMapWriter.write(probeFile, probeMap)
                project.logger.info(
                    "Omnivore: Wrote build-time probe map (${probeMap.getAllClassMaps().size} classes) to ${probeFile.absolutePath}"
                )
            }
        }

        // Wire dependency: probe map task runs after ASM transform tasks
        project.tasks.matching {
            it.name.contains("transformDebugClassesWithAsm") ||
                it.name.contains("transformReleaseClassesWithAsm")
        }.all { asmTask ->
            probeMapTask.configure { it.dependsOn(asmTask) }
        }
    }

    /**
     * Registers a task that creates a writable directory on the device before tests run.
     *
     * Android app processes cannot create directories in /data/local/tmp/ (owned by
     * shell:shell with mode 771). This task uses adb shell (running as shell user) to
     * create the directory with world-writable permissions so the app process can write
     * coverage data there during test execution.
     */
    private fun registerSetupTask(project: Project) {
        val setupTask = project.tasks.register("omnivoreSetupDevice") { task ->
            task.group = "omnivore"
            task.description = "Create writable coverage directory on connected Android device"

            task.doLast {
                val adb = resolveAdb(project) ?: return@doLast
                // Create directory and make it world-writable so the app process can write
                runCommand(adb, "shell", "mkdir", "-p", DEVICE_COVERAGE_DIR)
                runCommand(adb, "shell", "chmod", "777", DEVICE_COVERAGE_DIR)
            }
        }

        // Wire: connected test tasks depend on setup
        project.tasks.matching { it.name.startsWith("connected") && it.name.endsWith("AndroidTest") }
            .all { testTask ->
                testTask.dependsOn(setupTask)
            }
    }

    /**
     * Registers a task that extracts coverage data from the device after tests.
     *
     * Since AGP uninstalls the app INSIDE connectedDebugAndroidTest (before our
     * finalizer runs), and SELinux prevents app processes from writing to
     * /data/local/tmp/ (shell_data_file context), we extract coverage data from
     * logcat. OmnivoreTestListener outputs the .omnivore and .probes files as
     * base64-encoded strings with unique markers, which this task parses.
     *
     * Fallback strategies are attempted if logcat parsing fails:
     * - adb pull from /data/local/tmp/omnivore/ (in case device has permissive SELinux)
     * - run-as pull from app internal storage (in case app wasn't uninstalled)
     */
    private fun registerPullTask(project: Project) {
        val pullTask = project.tasks.register("omnivorePullCoverage") { task ->
            task.group = "omnivore"
            task.description = "Pull Omnivore coverage data from connected Android device"

            task.doLast {
                // Write to root project's build dir so OmnivoreReportTask finds the data.
                // The report task scans rootProject/build/omnivore/ for all coverage files.
                val outputDir = project.rootProject.layout.buildDirectory
                    .dir("omnivore/connectedAndroidTest")
                    .get().asFile
                outputDir.mkdirs()

                val adb = resolveAdb(project)
                if (adb == null) {
                    project.logger.warn("Omnivore: Could not find adb. Set ANDROID_HOME or add adb to PATH.")
                    return@doLast
                }

                project.logger.lifecycle("Pulling Omnivore coverage data from device...")

                var pulled = false

                // Strategy 1: Extract from logcat (most reliable — survives app uninstall + SELinux).
                // OmnivoreTestListener outputs coverage data as base64 between marker lines.
                pulled = extractFromLogcat(project, adb, outputDir)

                // Strategy 2: adb pull from /data/local/tmp/omnivore/
                if (!pulled) {
                    val pullExitCode = runCommand(adb, "pull", "$DEVICE_COVERAGE_DIR/.", outputDir.absolutePath)
                    if (pullExitCode == 0) {
                        val validFiles = outputDir.listFiles()?.filter {
                            (it.extension == "omnivore" || it.extension == "probes") && hasValidMagic(it)
                        } ?: emptyList()
                        if (validFiles.isNotEmpty()) pulled = true
                    }
                }

                // Strategy 3: run-as for debuggable apps (if app is still installed)
                if (!pulled) {
                    val packageName = resolvePackageName(project)
                    if (packageName != null) {
                        val coverageFiles = listOf("coverage.omnivore", "coverage.probes")
                        for (fileName in coverageFiles) {
                            val destFile = File(outputDir, fileName)
                            try {
                                val process = ProcessBuilder(
                                    adb, "exec-out", "run-as", packageName,
                                    "cat", "files/omnivore/$fileName"
                                ).redirectErrorStream(false).start()
                                destFile.outputStream().use { out ->
                                    process.inputStream.use { it.copyTo(out) }
                                }
                                val exitCode = process.waitFor()
                                if (exitCode == 0 && hasValidMagic(destFile)) {
                                    pulled = true
                                } else {
                                    destFile.delete()
                                }
                            } catch (e: Exception) {
                                project.logger.info("Omnivore: Failed to pull $fileName via run-as: ${e.message}")
                                destFile.delete()
                            }
                        }
                    }
                }

                if (pulled) {
                    val omnivoreFiles = outputDir.listFiles()?.filter {
                        it.extension == "omnivore" || it.extension == "probes"
                    } ?: emptyList()
                    project.logger.lifecycle(
                        "Pulled ${omnivoreFiles.size} coverage file(s) to ${outputDir.absolutePath}"
                    )
                } else {
                    project.logger.warn(
                        "Omnivore: Could not pull coverage data from device. " +
                            "Ensure OmnivoreTestListener ran in the test APK."
                    )
                }

                // Clean up device
                runCommand(adb, "shell", "rm", "-rf", DEVICE_COVERAGE_DIR)
            }
        }

        // Wire finalization: connected Android test tasks should finalize with pull
        project.tasks.matching { it.name.startsWith("connected") && it.name.endsWith("AndroidTest") }
            .all { testTask ->
                testTask.finalizedBy(pullTask)
            }
    }

    /**
     * Extract coverage data from logcat output.
     *
     * OmnivoreTestListener outputs .omnivore and .probes files as base64 between
     * unique marker lines (e.g. OMNIVORE_EXEC_DATA_START / OMNIVORE_EXEC_DATA_END).
     * This approach is immune to filesystem permissions and app uninstallation.
     */
    private fun extractFromLogcat(project: Project, adb: String, outputDir: File): Boolean {
        try {
            val process = ProcessBuilder(adb, "logcat", "-d", "-s", "System.err:W")
                .redirectErrorStream(true)
                .start()
            val logcatOutput = process.inputStream.bufferedReader().readText()
            process.waitFor()

            var extracted = false

            // Extract execution data
            val execData = extractBase64Block(logcatOutput, "OMNIVORE_EXEC_DATA_START", "OMNIVORE_EXEC_DATA_END")
            if (execData != null) {
                val bytes = java.util.Base64.getDecoder().decode(execData)
                val destFile = File(outputDir, "coverage.omnivore")
                destFile.writeBytes(bytes)
                if (hasValidMagic(destFile)) {
                    project.logger.info("Omnivore: Extracted execution data from logcat (${bytes.size} bytes)")
                    extracted = true
                } else {
                    destFile.delete()
                }
            }

            // Extract probe map
            val probeData = extractBase64Block(logcatOutput, "OMNIVORE_PROBE_DATA_START", "OMNIVORE_PROBE_DATA_END")
            if (probeData != null) {
                val bytes = java.util.Base64.getDecoder().decode(probeData)
                val destFile = File(outputDir, "coverage.probes")
                destFile.writeBytes(bytes)
                if (hasValidMagic(destFile)) {
                    project.logger.info("Omnivore: Extracted probe map from logcat (${bytes.size} bytes)")
                    extracted = true
                } else {
                    destFile.delete()
                }
            }

            return extracted
        } catch (e: Exception) {
            project.logger.info("Omnivore: Failed to extract from logcat: ${e.message}")
            return false
        }
    }

    /**
     * Extract the base64 content between start and end markers from logcat output.
     * Logcat lines have a prefix format like "03-19 22:27:32.585 17115 17130 W System.err: "
     * which we strip before extracting the base64 data.
     */
    private fun extractBase64Block(logcat: String, startMarker: String, endMarker: String): String? {
        val lines = logcat.lines()
        var capturing = false
        val data = StringBuilder()

        for (line in lines) {
            // Strip logcat prefix — everything after "System.err: " is the actual output
            val content = line.substringAfter("System.err: ", missingDelimiterValue = "").trim()
            if (content.isEmpty()) continue

            if (content == endMarker) {
                if (capturing && data.isNotEmpty()) return data.toString()
                capturing = false
            } else if (capturing) {
                data.append(content)
            } else if (content == startMarker) {
                capturing = true
                data.clear()
            }
        }
        return null
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

    /**
     * Resolve the application's package name from AGP's android extension.
     */
    private fun resolvePackageName(project: Project): String? {
        return try {
            val androidExtension = project.extensions.findByName("android") ?: return null
            val defaultConfig = androidExtension.javaClass.getMethod("getDefaultConfig").invoke(androidExtension)
            defaultConfig.javaClass.getMethod("getApplicationId").invoke(defaultConfig) as? String
        } catch (_: Exception) {
            // Try namespace as fallback
            try {
                val androidExtension = project.extensions.findByName("android") ?: return null
                androidExtension.javaClass.getMethod("getNamespace").invoke(androidExtension) as? String
            } catch (_: Exception) { null }
        }
    }

    /**
     * Resolve the slim runtime JAR (Omnivore classes only, no bundled libraries).
     *
     * Uses a multi-strategy approach inspired by JaCoCo's agent resolution:
     *
     * 1. **Composite build**: Search Gradle's included builds for the agent module's
     *    build output. The runtime JAR lives in `omnivore-agent/build/libs/`.
     *
     * 2. **Gradle configuration**: Create a detached configuration with the runtime
     *    classifier and let Gradle resolve from repositories (Maven Central, etc.).
     *
     * 3. **Classloader fallback**: Find the fat JAR via protectionDomain, then extract
     *    Omnivore classes into a slim temp JAR (handles edge cases like Gradle
     *    transform caches where the runtime JAR isn't co-located).
     */
    private fun resolveRuntimeJar(project: Project): File? {
        // Strategy 1: Composite build — find in included build output
        resolveFromIncludedBuild(project)?.let { return it }

        // Strategy 2: Gradle configuration — resolve from repositories
        resolveViaGradleConfiguration(project)?.let { return it }

        // Strategy 3: Extract from fat JAR on classpath
        extractRuntimeFromFatJar(project)?.let { return it }

        return null
    }

    /**
     * Search Gradle's included builds for the runtime JAR in the agent module's build output.
     * This is the primary path for composite builds (development workflow).
     */
    private fun resolveFromIncludedBuild(project: Project): File? {
        for (build in project.gradle.includedBuilds) {
            val libsDir = File(build.projectDir, "omnivore-agent/build/libs")
            if (!libsDir.isDirectory) continue
            val runtimeJar = libsDir.listFiles()
                ?.firstOrNull { it.name.contains("runtime") && it.name.endsWith(".jar") }
            if (runtimeJar != null) {
                project.logger.info("Omnivore: Resolved runtime JAR from included build: ${runtimeJar.absolutePath}")
                return runtimeJar
            }
        }
        return null
    }

    /**
     * Resolve the runtime JAR via a detached Gradle configuration.
     * This mirrors JaCoCo's `jacocoAgent` configuration pattern and is the
     * primary path for published plugin scenarios.
     */
    private fun resolveViaGradleConfiguration(project: Project): File? {
        val props = loadVersionProperties() ?: return null
        val group = props.getProperty("group") ?: return null
        val artifactId = props.getProperty("artifactId") ?: return null
        val version = props.getProperty("version") ?: return null

        return try {
            val dep = project.dependencies.create("$group:$artifactId:$version:runtime")
            val config = project.configurations.detachedConfiguration(dep)
            config.isTransitive = false
            val files = config.resolve()
            files.firstOrNull { it.name.contains("runtime") && it.name.endsWith(".jar") }
        } catch (e: Exception) {
            project.logger.info("Omnivore: Gradle configuration resolution failed: ${e.message}")
            null
        }
    }

    /**
     * Last resort: find the fat agent JAR on the classloader and extract only
     * Omnivore classes into a slim temporary JAR. This handles cases where the
     * fat JAR lives in a Gradle transforms cache and the runtime JAR isn't nearby.
     */
    private fun extractRuntimeFromFatJar(project: Project): File? {
        val fatJar = try {
            val uri = Class.forName("com.jkjamies.omnivore.agent.OmnivoreAgent")
                .protectionDomain.codeSource?.location?.toURI()
            uri?.let { File(it) }?.takeIf { it.isFile }
        } catch (_: Exception) { null } ?: return null

        return try {
            val outputDir = project.layout.buildDirectory.dir("omnivore/runtime").get().asFile
            outputDir.mkdirs()
            val runtimeJar = File(outputDir, "omnivore-agent-runtime.jar")

            java.util.jar.JarOutputStream(runtimeJar.outputStream()).use { jos ->
                java.util.jar.JarFile(fatJar).use { jf ->
                    for (entry in jf.entries()) {
                        // Only include Omnivore's own classes
                        if (entry.name.startsWith("com/jkjamies/omnivore/")) {
                            jos.putNextEntry(java.util.jar.JarEntry(entry.name))
                            jf.getInputStream(entry).use { it.copyTo(jos) }
                            jos.closeEntry()
                        }
                    }
                }
            }

            project.logger.info("Omnivore: Extracted runtime JAR from fat JAR: ${runtimeJar.absolutePath}")
            runtimeJar
        } catch (e: Exception) {
            project.logger.info("Omnivore: Failed to extract runtime from fat JAR: ${e.message}")
            null
        }
    }

    /**
     * Validate that a coverage file starts with the expected binary magic header.
     * .omnivore files start with "OMNIVORE", .probes files start with "OMNIPROB".
     * This catches cases where error messages (e.g. "run-as: unknown package")
     * are written to the file instead of actual binary data.
     */
    private fun hasValidMagic(file: File): Boolean {
        if (!file.isFile || file.length() < 8) return false
        return try {
            val header = ByteArray(8)
            file.inputStream().use { it.read(header) }
            val magic = String(header, Charsets.US_ASCII)
            magic == "OMNIVORE" || magic == "OMNIPROB"
        } catch (_: Exception) { false }
    }

    /**
     * Run a command via ProcessBuilder, returning the exit code.
     * Replaces Project.exec() which was removed in Gradle 9.
     */
    private fun runCommand(vararg args: String): Int {
        return try {
            ProcessBuilder(*args)
                .inheritIO()
                .start()
                .waitFor()
        } catch (_: Exception) {
            -1
        }
    }

    private fun loadVersionProperties(): Properties? {
        return try {
            val stream = InstrumentedTestConfigurator::class.java.classLoader
                .getResourceAsStream(VERSION_PROPS_RESOURCE) ?: return null
            Properties().apply { load(stream) }
        } catch (_: Exception) { null }
    }
}
