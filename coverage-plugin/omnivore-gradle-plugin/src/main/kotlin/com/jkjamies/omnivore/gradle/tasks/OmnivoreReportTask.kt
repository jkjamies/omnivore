package com.jkjamies.omnivore.gradle.tasks

import com.jkjamies.omnivore.agent.model.CoverageTarget
import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.reporter.CoverageAnalyzer
import com.jkjamies.omnivore.agent.reporter.HtmlReportWriter
import com.jkjamies.omnivore.agent.reporter.JsonReportWriter
import com.jkjamies.omnivore.agent.reporter.MarkdownReportWriter
import com.jkjamies.omnivore.agent.runtime.ExecutionDataReader
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeMapReader
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import java.io.File

/**
 * Gradle task that generates Omnivore coverage reports from execution data.
 *
 * Reads .omnivore execution data and .probes probe map files produced by the
 * agent during test runs, then generates coverage reports in configured formats.
 */
abstract class OmnivoreReportTask : DefaultTask() {

    @get:InputDirectory
    @get:Optional
    abstract val executionDataDir: DirectoryProperty

    @get:OutputDirectory
    abstract val reportDir: DirectoryProperty

    @get:Input
    @get:Optional
    abstract val jsonEnabled: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val htmlEnabled: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val markdownEnabled: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val projectId: Property<String>

    @get:Input
    @get:Optional
    abstract val projectName: Property<String>

    @get:Input
    @get:Optional
    abstract val dependenciesEnabled: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val dependenciesIncludeExternal: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val dependenciesIncludeTestDeps: Property<Boolean>

    /** Set by the plugin if dependency resolution succeeds. Not a task input. */
    @get:Internal
    var resolvedDependencyGraph: DependencyGraph? = null

    init {
        executionDataDir.convention(project.layout.buildDirectory.dir("omnivore"))
        reportDir.convention(project.layout.buildDirectory.dir("reports/omnivore"))
        jsonEnabled.convention(true)
        htmlEnabled.convention(true)
        markdownEnabled.convention(false)
        projectId.convention(project.name)
        projectName.convention(project.name)
        dependenciesEnabled.convention(false)
        dependenciesIncludeExternal.convention(false)
        dependenciesIncludeTestDeps.convention(false)
    }

    @TaskAction
    fun generateReport() {
        val dataDir = executionDataDir.get().asFile
        if (!dataDir.exists()) {
            logger.warn("No Omnivore execution data found at ${dataDir.absolutePath}. Run tests first.")
            return
        }

        // Find execution data files
        val execFiles = dataDir.walkTopDown()
            .filter { it.extension == "omnivore" }
            .toList()

        if (execFiles.isEmpty()) {
            logger.warn("No .omnivore files found in ${dataDir.absolutePath}. Run tests with Omnivore coverage enabled.")
            return
        }

        // Find probe map files
        val probeFiles = dataDir.walkTopDown()
            .filter { it.extension == "probes" }
            .toList()

        if (probeFiles.isEmpty()) {
            logger.warn("No .probes files found in ${dataDir.absolutePath}. Agent may not have recorded probe mappings.")
            return
        }

        logger.lifecycle("Found ${execFiles.size} execution data file(s) and ${probeFiles.size} probe map file(s)")

        // Determine coverage target based on data sources
        val hasInstrumented = execFiles.any { it.absolutePath.contains("connectedAndroidTest") }
        val hasUnit = execFiles.any { !it.absolutePath.contains("connectedAndroidTest") }
        val target = when {
            hasUnit && hasInstrumented -> CoverageTarget.COMPOSITE
            hasInstrumented -> CoverageTarget.ANDROID_INSTRUMENTED
            else -> CoverageTarget.JVM_UNIT
        }
        logger.lifecycle("Coverage target: $target")

        // Merge all execution data files
        val mergedStore = ExecutionDataStore()
        for (execFile in execFiles) {
            val fileStore = ExecutionDataReader.read(execFile)
            for (data in fileStore.getAllData()) {
                val probes = mergedStore.getOrCreateProbes(data.classId, data.className, data.probes.size)
                for (i in data.probes.indices) {
                    if (data.probes[i]) probes[i] = true
                }
            }
        }

        // Merge all probe maps
        val mergedProbeMap = ProbeMap()
        for (probeFile in probeFiles) {
            val fileProbeMap = ProbeMapReader.read(probeFile)
            for (classMap in fileProbeMap.getAllClassMaps()) {
                val target = mergedProbeMap.getOrCreateClassMap(
                    classMap.classId, classMap.className, classMap.sourceFile
                )
                for (probe in classMap.getProbes()) {
                    target.addProbe(probe.probeIndex, probe.lineNumber, probe.methodName, probe.methodDesc, probe.type)
                }
            }
        }

        // Analyze
        val result = CoverageAnalyzer.analyze(mergedStore, mergedProbeMap)

        logger.lifecycle(
            "Coverage: %.1f%% lines (%d/%d), %.1f%% branches (%d/%d)".format(
                result.summary.lineRate * 100,
                result.summary.linesCovered,
                result.summary.linesTotal,
                result.summary.branchRate * 100,
                result.summary.branchesCovered,
                result.summary.branchesTotal,
            )
        )

        val outputDir = reportDir.get().asFile
        outputDir.mkdirs()

        // Resolve dependency graph if enabled
        val depGraph = if (dependenciesEnabled.getOrElse(false)) {
            try {
                com.jkjamies.omnivore.gradle.configuration.DependencyGraphResolver.resolve(
                    project = project,
                    includeExternal = dependenciesIncludeExternal.getOrElse(false),
                    includeTestDeps = dependenciesIncludeTestDeps.getOrElse(false),
                )
            } catch (e: Exception) {
                logger.warn("Failed to resolve dependency graph: ${e.message}")
                null
            }
        } else {
            resolvedDependencyGraph
        }

        if (depGraph != null) {
            logger.lifecycle("Dependency graph: ${depGraph.modules.size} modules, ${depGraph.edges.size} edges")
        }

        // Generate JSON report
        if (jsonEnabled.get()) {
            val jsonFile = File(outputDir, "omnivore-report.json")
            JsonReportWriter.write(
                outputFile = jsonFile,
                analysisResult = result,
                projectId = projectId.get(),
                projectName = projectName.get(),
                target = target,
                dependencyGraph = depGraph,
            )
            logger.lifecycle("JSON report: ${jsonFile.absolutePath}")
        }

        // Generate HTML report
        if (htmlEnabled.get()) {
            val htmlFile = File(outputDir, "index.html")
            HtmlReportWriter.write(htmlFile, result)
            logger.lifecycle("HTML report: ${htmlFile.absolutePath}")
        }

        // Generate Markdown report
        if (markdownEnabled.get()) {
            val mdFile = File(outputDir, "coverage.md")
            MarkdownReportWriter.write(mdFile, result)
            logger.lifecycle("Markdown report: ${mdFile.absolutePath}")
        }

        logger.lifecycle("Omnivore report generated at ${outputDir.absolutePath}")
    }
}
