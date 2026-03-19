package com.jkjamies.omnivore.gradle.tasks

import com.jkjamies.omnivore.agent.model.CoverageTarget
import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.model.FileCoverage
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
import org.gradle.internal.logging.text.StyledTextOutput
import org.gradle.internal.logging.text.StyledTextOutput.Style
import org.gradle.internal.logging.text.StyledTextOutputFactory
import java.io.File

/**
 * Gradle task that generates Omnivore coverage reports from execution data.
 *
 * Reads .omnivore execution data and .probes probe map files produced by the
 * agent during test runs, then generates coverage reports in configured formats.
 *
 * When both unit and instrumented test data is present, reports them as
 * separate sections with independent thresholds rather than merging.
 */
abstract class OmnivoreReportTask : DefaultTask() {

    @get:Internal
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
        // Always run so users see coverage output — the task is fast
        outputs.upToDateWhen { false }

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

    /** A target-specific slice of coverage data. */
    private data class TargetCoverage(
        val target: CoverageTarget,
        val result: CoverageAnalyzer.AnalysisResult,
    )

    @TaskAction
    fun generateReport() {
        val dataDir = executionDataDir.get().asFile
        if (!dataDir.exists()) {
            logger.warn("No Omnivore execution data found at ${dataDir.absolutePath}. Run tests first.")
            return
        }

        // Find all data files
        val allExecFiles = dataDir.walkTopDown().filter { it.extension == "omnivore" }.toList()
        val allProbeFiles = dataDir.walkTopDown().filter { it.extension == "probes" }.toList()

        if (allExecFiles.isEmpty()) {
            logger.warn("No .omnivore files found in ${dataDir.absolutePath}. Run tests with Omnivore coverage enabled.")
            return
        }
        if (allProbeFiles.isEmpty()) {
            logger.warn("No .probes files found in ${dataDir.absolutePath}. Agent may not have recorded probe mappings.")
            return
        }

        // Partition files by source: unit vs instrumented
        val unitExecFiles = allExecFiles.filter { !it.absolutePath.contains("connectedAndroidTest") }
        val instrumentedExecFiles = allExecFiles.filter { it.absolutePath.contains("connectedAndroidTest") }
        val unitProbeFiles = allProbeFiles.filter { !it.absolutePath.contains("connectedAndroidTest") }
        val instrumentedProbeFiles = allProbeFiles.filter { it.absolutePath.contains("connectedAndroidTest") }

        // Analyze each target independently
        val targets = mutableListOf<TargetCoverage>()

        if (unitExecFiles.isNotEmpty() && unitProbeFiles.isNotEmpty()) {
            val (store, probeMap) = mergeData(unitExecFiles, unitProbeFiles)
            targets.add(TargetCoverage(CoverageTarget.JVM_UNIT, CoverageAnalyzer.analyze(store, probeMap)))
        }

        if (instrumentedExecFiles.isNotEmpty() && instrumentedProbeFiles.isNotEmpty()) {
            val (store, probeMap) = mergeData(instrumentedExecFiles, instrumentedProbeFiles)
            targets.add(TargetCoverage(CoverageTarget.ANDROID_INSTRUMENTED, CoverageAnalyzer.analyze(store, probeMap)))
        }

        if (targets.isEmpty()) {
            logger.warn("No matching execution data and probe maps found.")
            return
        }

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

        // For report files, use the primary target (or first one)
        val primaryTarget = targets.first()
        // For JSON, merge all data for backwards compatibility
        val mergedResult = if (targets.size > 1) {
            val (store, probeMap) = mergeData(allExecFiles, allProbeFiles)
            CoverageAnalyzer.analyze(store, probeMap)
        } else {
            primaryTarget.result
        }

        val reportTarget = if (targets.size > 1) CoverageTarget.COMPOSITE else primaryTarget.target

        // Generate reports
        val reportFormats = mutableListOf<String>()
        if (jsonEnabled.get()) {
            val jsonFile = File(outputDir, "omnivore-report.json")
            JsonReportWriter.write(
                outputFile = jsonFile,
                analysisResult = mergedResult,
                projectId = projectId.get(),
                projectName = projectName.get(),
                target = reportTarget,
                dependencyGraph = depGraph,
            )
            reportFormats.add("json")
        }
        if (htmlEnabled.get()) {
            val htmlFile = File(outputDir, "index.html")
            HtmlReportWriter.write(htmlFile, mergedResult)
            reportFormats.add("html")
        }
        if (markdownEnabled.get()) {
            val mdFile = File(outputDir, "coverage.md")
            MarkdownReportWriter.write(mdFile, mergedResult)
            reportFormats.add("markdown")
        }

        // Print output
        printReport(targets, depGraph, reportFormats, outputDir)
    }

    private fun mergeData(
        execFiles: List<File>,
        probeFiles: List<File>,
    ): Pair<ExecutionDataStore, ProbeMap> {
        val store = ExecutionDataStore()
        for (execFile in execFiles) {
            val fileStore = ExecutionDataReader.read(execFile)
            for (data in fileStore.getAllData()) {
                val probes = store.getOrCreateProbes(data.classId, data.className, data.probes.size)
                for (i in data.probes.indices) {
                    if (data.probes[i]) probes[i] = true
                }
            }
        }
        val probeMap = ProbeMap()
        for (probeFile in probeFiles) {
            val fileProbeMap = ProbeMapReader.read(probeFile)
            for (classMap in fileProbeMap.getAllClassMaps()) {
                val target = probeMap.getOrCreateClassMap(
                    classMap.classId, classMap.className, classMap.sourceFile
                )
                for (probe in classMap.getProbes()) {
                    target.addProbe(probe.probeIndex, probe.lineNumber, probe.methodName, probe.methodDesc, probe.type)
                }
            }
        }
        return store to probeMap
    }

    // -- Pretty output --

    private fun printReport(
        targets: List<TargetCoverage>,
        depGraph: DependencyGraph?,
        reportFormats: List<String>,
        outputDir: File,
    ) {
        val out = services.get(StyledTextOutputFactory::class.java)
            .create("omnivore")

        out.println()
        out.style(Style.Header).text("  Omnivore Coverage Report").println()
        out.println()

        for ((index, tc) in targets.withIndex()) {
            printTargetSection(out, tc)
            if (index < targets.size - 1) {
                out.println()
            }
        }

        // Dependency graph
        if (depGraph != null && depGraph.modules.isNotEmpty()) {
            out.style(Style.Description).text("  Dependencies: ${depGraph.modules.size} modules, ${depGraph.edges.size} edges").println()
        }

        // Reports
        out.style(Style.Description).text("  Reports: ").style(Style.Info).text(outputDir.absolutePath).println()
        out.style(Style.Description).text("  Formats: ").style(Style.Normal).text(reportFormats.joinToString(", ")).println()
        out.println()
    }

    private fun printTargetSection(out: StyledTextOutput, tc: TargetCoverage) {
        val s = tc.result.summary
        val files = tc.result.files

        val (label, thresholds) = when (tc.target) {
            CoverageTarget.JVM_UNIT -> "Unit Tests" to Thresholds(green = 80.0, yellow = 50.0)
            CoverageTarget.ANDROID_INSTRUMENTED -> "Instrumented Tests" to Thresholds(green = 50.0, yellow = 25.0)
            CoverageTarget.IOS_UNIT -> "iOS Unit Tests" to Thresholds(green = 80.0, yellow = 50.0)
            CoverageTarget.KOTLIN_NATIVE -> "Kotlin/Native Tests" to Thresholds(green = 80.0, yellow = 50.0)
            CoverageTarget.COMPOSITE -> "All Tests" to Thresholds(green = 70.0, yellow = 40.0)
        }

        out.style(Style.Description).text("  ── $label ").text("─".repeat((48 - label.length).coerceAtLeast(2)))
        out.style(Style.Info).text("  ${files.size} files").println()
        out.println()

        // Summary bars
        val linesPct = s.lineRate * 100
        val branchPct = s.branchRate * 100
        out.text("  Lines      ")
        printBar(out, linesPct, thresholds)
        out.text("  ${fmt(linesPct)}  ${s.linesCovered}/${s.linesTotal}").println()
        out.text("  Branches   ")
        printBar(out, branchPct, thresholds)
        out.text("  ${fmt(branchPct)}  ${s.branchesCovered}/${s.branchesTotal}").println()
        out.println()

        // File table
        val maxPath = (files.maxOfOrNull { displayPath(it.path).length } ?: 20).coerceIn(20, 52)
        out.style(Style.Normal).text("  ${"File".padEnd(maxPath)}   Lines   Branches").println()
        out.style(Style.Normal).text("  ${"─".repeat(maxPath)}  ───────  ────────").println()

        for (file in files) {
            val path = displayPath(file.path)
            val lPct = file.lineRate * 100
            val bPct = file.branchRate * 100
            val covered = file.lines.count { it.hitCount > 0 }
            val total = file.lines.size
            out.text("  ${path.padEnd(maxPath)}  ")
            out.style(styleFor(lPct, thresholds)).text(fmt(lPct).padStart(6))
            out.style(Style.Normal).text("  ")
            out.style(styleFor(bPct, thresholds)).text(fmt(bPct).padStart(6))
            out.style(Style.Info).text("   $covered/$total").println()
        }

        out.println()
    }

    private data class Thresholds(val green: Double, val yellow: Double)

    private fun printBar(out: StyledTextOutput, pct: Double, thresholds: Thresholds) {
        val width = 24
        val filled = ((pct / 100.0) * width).toInt().coerceIn(0, width)
        val empty = width - filled
        out.style(styleFor(pct, thresholds)).text("█".repeat(filled))
        out.style(Style.Normal).text("░".repeat(empty))
    }

    private fun fmt(pct: Double): String = "%5.1f%%".format(pct)

    private fun styleFor(pct: Double, thresholds: Thresholds): Style = when {
        pct >= thresholds.green -> Style.SuccessHeader   // green
        pct >= thresholds.yellow -> Style.Description     // yellow
        else -> Style.Failure                              // red
    }

    private fun displayPath(path: String): String {
        val parts = path.split("/")
        return if (parts.size > 2) {
            "\u2026/" + parts.takeLast(2).joinToString("/")
        } else {
            path
        }
    }
}
