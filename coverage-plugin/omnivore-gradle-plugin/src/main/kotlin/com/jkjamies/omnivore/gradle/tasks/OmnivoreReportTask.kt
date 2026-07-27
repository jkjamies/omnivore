package com.jkjamies.omnivore.gradle.tasks

import com.jkjamies.omnivore.agent.model.CoverageTarget
import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.model.FileCoverage
import com.jkjamies.omnivore.agent.instrumentation.GlobPattern
import com.jkjamies.omnivore.agent.reporter.CoverageAnalyzer
import com.jkjamies.omnivore.agent.reporter.HtmlReportWriter
import com.jkjamies.omnivore.agent.reporter.JsonReportWriter
import com.jkjamies.omnivore.agent.reporter.MarkdownReportWriter
import com.jkjamies.omnivore.agent.runtime.ExecutionDataReader
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.InstrumentationStatsIo
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeMapReader
import com.jkjamies.omnivore.gradle.GraphFormat
import com.jkjamies.omnivore.gradle.configuration.DependencyGraphWriter
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.*
import org.gradle.internal.logging.text.StyledTextOutput
import org.gradle.internal.logging.text.StyledTextOutput.Style
import org.gradle.internal.logging.text.StyledTextOutputFactory
import org.gradle.work.DisableCachingByDefault
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
@DisableCachingByDefault(
    because = "Reads .omnivore/.probes files written as a side effect of test execution, " +
        "which are not declared inputs. A cache hit would serve a report for a different run.",
)
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

    @get:Input
    @get:Optional
    abstract val unitTestExcludes: ListProperty<String>

    @get:Input
    @get:Optional
    abstract val instrumentedTestExcludes: ListProperty<String>

    @get:Input
    @get:Optional
    abstract val excludeFiles: ListProperty<String>

    @get:Input
    @get:Optional
    abstract val excludeMethods: ListProperty<String>

    @get:Input
    @get:Optional
    abstract val excludeAnnotations: ListProperty<String>

    @get:Input
    @get:Optional
    abstract val localGraphEnabled: Property<Boolean>

    @get:Input
    @get:Optional
    abstract val localGraphFormat: Property<String>

    @get:Internal
    abstract val localGraphOutputFile: Property<java.io.File>

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
        unitTestExcludes.convention(emptyList())
        instrumentedTestExcludes.convention(emptyList())
        excludeFiles.convention(emptyList())
        excludeMethods.convention(emptyList())
        excludeAnnotations.convention(emptyList())
        localGraphEnabled.convention(false)
        localGraphFormat.convention("MERMAID")
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
            filterProbeMap(probeMap, CoverageTarget.JVM_UNIT)
            targets.add(TargetCoverage(CoverageTarget.JVM_UNIT, CoverageAnalyzer.analyze(store, probeMap)))
        }

        if (instrumentedExecFiles.isNotEmpty() && instrumentedProbeFiles.isNotEmpty()) {
            val (store, probeMap) = mergeData(instrumentedExecFiles, instrumentedProbeFiles)
            filterProbeMap(probeMap, CoverageTarget.ANDROID_INSTRUMENTED)
            targets.add(TargetCoverage(CoverageTarget.ANDROID_INSTRUMENTED, CoverageAnalyzer.analyze(store, probeMap)))
        }

        if (targets.isEmpty()) {
            logger.warn("No matching execution data and probe maps found.")
            return
        }

        val outputDir = reportDir.get().asFile
        outputDir.mkdirs()

        // Use pre-resolved dependency graph (resolved at configuration time to avoid
        // accessing Project at execution time, which breaks Gradle configuration cache)
        val depGraph = resolvedDependencyGraph

        // Write local dependency graph file if enabled
        if (localGraphEnabled.getOrElse(false) && depGraph == null) {
            logger.warn("Omnivore: Local graph enabled but no dependency graph resolved. Ensure dependencies.enabled is true.")
        }
        if (localGraphEnabled.getOrElse(false) && depGraph != null && depGraph.modules.isNotEmpty()) {
            val format = try {
                GraphFormat.valueOf(localGraphFormat.getOrElse("MERMAID"))
            } catch (_: IllegalArgumentException) {
                GraphFormat.MERMAID
            }
            val graphFile = if (localGraphOutputFile.isPresent) {
                localGraphOutputFile.get()
            } else {
                File(outputDir, "dependency-graph.${DependencyGraphWriter.defaultExtension(format)}")
            }
            DependencyGraphWriter.write(graphFile, depGraph, format)
            logger.lifecycle("Omnivore: Wrote dependency graph to ${graphFile.absolutePath}")
        }

        // Merge all target data for the combined local HTML/markdown view.
        val mergedResult = if (targets.size > 1) {
            val (store, probeMap) = mergeData(allExecFiles, allProbeFiles)
            CoverageAnalyzer.analyze(store, probeMap)
        } else {
            targets.first().result
        }

        // Generate reports. Each coverage target gets its own omnivore-report.json so
        // the dashboard tracks it as a separate series (Unit vs. Instrumented) rather
        // than a single blended number. A single-target run writes
        //   build/reports/omnivore/omnivore-report.json
        // a multi-target run writes one per target under a target-named subdirectory:
        //   build/reports/omnivore/<target>/omnivore-report.json
        val reportFormats = mutableListOf<String>()
        if (jsonEnabled.get()) {
            for (tc in targets) {
                val targetOutputDir = if (targets.size > 1) {
                    File(outputDir, tc.target.name.lowercase().replace("_", "-")).apply { mkdirs() }
                } else {
                    outputDir
                }
                JsonReportWriter.write(
                    outputFile = File(targetOutputDir, "omnivore-report.json"),
                    analysisResult = tc.result,
                    projectId = projectId.get(),
                    projectName = projectName.get(),
                    target = tc.target,
                    dependencyGraph = depGraph,
                )
            }
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
        printReport(targets, depGraph, reportFormats, outputDir, readInstrumentationSummary(dataDir))
    }

    private fun mergeData(
        execFiles: List<File>,
        probeFiles: List<File>,
    ): Pair<ExecutionDataStore, ProbeMap> {
        val store = ExecutionDataStore()
        var mismatchedClasses = 0
        for (execFile in execFiles) {
            val fileStore = ExecutionDataReader.read(execFile)
            for (data in fileStore.getAllData()) {
                val probes = store.getOrCreateProbes(data.classId, data.className, data.probes.size)
                // Probe arrays for the same class can differ in length across
                // files: a class ID is a hash of the class *name*, so a stale
                // .omnivore file from before a recompile is keyed identically
                // but was instrumented with a different probe count. Copying
                // blind used to throw ArrayIndexOutOfBoundsException and fail
                // the whole report; clamp to the shorter array instead and
                // report how much was skipped.
                if (data.probes.size != probes.size) {
                    mismatchedClasses++
                    logger.debug(
                        "Omnivore: probe count mismatch for {} ({} vs {}) — merging the common prefix",
                        data.className, data.probes.size, probes.size,
                    )
                }
                val shared = minOf(data.probes.size, probes.size)
                for (i in 0 until shared) {
                    if (data.probes[i]) probes[i] = true
                }
            }
        }
        if (mismatchedClasses > 0) {
            logger.warn(
                "Omnivore: $mismatchedClasses class(es) had inconsistent probe counts across execution " +
                    "data files. This usually means stale data from a previous build — run a clean " +
                    "build if coverage looks wrong."
            )
        }
        val probeMap = ProbeMap()
        val seenProbes = mutableMapOf<Long, MutableSet<Int>>()
        val unreadableProbeFiles = mutableListOf<String>()
        for (probeFile in probeFiles) {
            // A probe map written by an older Omnivore is rejected by the
            // reader, because v3 moved branch probes onto control-flow edges
            // and the indices no longer mean the same thing. Skip the file
            // rather than letting the exception abort the whole report: one
            // stale artifact in build/ should not make the task unrunnable,
            // and the guard below still fails loudly if nothing usable is left.
            val fileProbeMap = try {
                ProbeMapReader.read(probeFile)
            } catch (e: Exception) {
                unreadableProbeFiles += "${probeFile.name}: ${e.message}"
                continue
            }
            for (classMap in fileProbeMap.getAllClassMaps()) {
                val target = probeMap.getOrCreateClassMap(
                    classMap.classId, classMap.className, classMap.sourceFile
                )
                val seen = seenProbes.getOrPut(classMap.classId) { mutableSetOf() }
                for (probe in classMap.getProbes()) {
                    // Two probe files can describe the same class (multiple test
                    // tasks writing into the same directory). Adding an entry
                    // per occurrence double-counted every branch, since the
                    // analyzer counts one branch per BRANCH entry.
                    if (!seen.add(probe.probeIndex)) continue
                    target.addProbe(
                        probe.probeIndex,
                        probe.lineNumber,
                        probe.methodName,
                        probe.methodDesc,
                        probe.type,
                        // isComposable and branchGroup were being dropped here.
                        // Losing isComposable silently disabled the "auto-exclude
                        // pure Compose classes" filter, because
                        // isAllMethodsComposable() can never be true once every
                        // entry says false.
                        probe.isComposable,
                        probe.branchGroup,
                    )
                }
            }
        }

        if (unreadableProbeFiles.isNotEmpty()) {
            logger.warn(
                "Omnivore: skipped ${unreadableProbeFiles.size} unreadable probe map(s):\n  " +
                    unreadableProbeFiles.joinToString("\n  ")
            )
        }
        if (probeMap.isEmpty()) {
            throw TaskExecutionException(
                this,
                RuntimeException(
                    "No usable probe maps: all ${probeFiles.size} .probes file(s) were written by a " +
                        "different version of Omnivore. Run a clean build to regenerate coverage data."
                )
            )
        }

        dropStaleClasses(store, probeMap)
        return store to probeMap
    }

    /**
     * Drop classes whose probe map and execution data disagree on probe count.
     *
     * Class IDs are derived from the class *name*, so execution data left over
     * from before a recompile keys to the same entry as the current probe map
     * while describing a different instrumentation. Correlating them produces
     * coverage attributed to the wrong source lines — plausible-looking numbers
     * with nothing to indicate they are wrong. Dropping the class is the honest
     * outcome; the warning points at the fix.
     */
    private fun dropStaleClasses(store: ExecutionDataStore, probeMap: ProbeMap) {
        val stale = mutableListOf<String>()
        for (classMap in probeMap.getAllClassMaps()) {
            val data = store.getData(classMap.classId) ?: continue
            val mappedProbes = classMap.getProbes().size
            if (mappedProbes > data.probes.size) {
                stale += classMap.className
                probeMap.removeClassMap(classMap.classId)
            }
        }
        if (stale.isNotEmpty()) {
            logger.warn(
                "Omnivore: dropped ${stale.size} class(es) whose execution data predates the current " +
                    "instrumentation (e.g. ${stale.take(3).joinToString()}). Run a clean build to " +
                    "include them."
            )
        }
    }

    /**
     * Filter the probe map before analysis for a specific target:
     * - For JVM_UNIT: auto-exclude pure Compose classes (all methods @Composable)
     * - Apply per-target exclude patterns
     */
    private fun filterProbeMap(probeMap: ProbeMap, target: CoverageTarget) {
        val excludePatterns = when (target) {
            CoverageTarget.JVM_UNIT -> unitTestExcludes.getOrElse(emptyList())
            CoverageTarget.ANDROID_INSTRUMENTED -> instrumentedTestExcludes.getOrElse(emptyList())
            else -> emptyList()
        }
        val filePatterns = excludeFiles.getOrElse(emptyList())
        val methodPatterns = excludeMethods.getOrElse(emptyList())

        val toRemove = mutableListOf<Long>()
        var excludedComposeCount = 0

        for (classMap in probeMap.getAllClassMaps()) {
            val dotName = classMap.className.replace('/', '.')

            // Auto-exclude pure Compose classes for JVM_UNIT (zero config)
            if (target == CoverageTarget.JVM_UNIT && classMap.isAllMethodsComposable()) {
                toRemove.add(classMap.classId)
                excludedComposeCount++
                continue
            }

            // Per-target exclude patterns (class name)
            if (excludePatterns.any { patternMatches(it, dotName) }) {
                toRemove.add(classMap.classId)
                continue
            }

            // Source file path exclusions
            val sourceFile = classMap.sourceFile
            if (sourceFile != null && filePatterns.any { patternMatches(it, sourceFile) }) {
                toRemove.add(classMap.classId)
                continue
            }

            // Method-level exclusions: remove matching probes from the class map
            if (methodPatterns.isNotEmpty()) {
                val probesToRemove = classMap.getProbes().filter { probe ->
                    methodPatterns.any { patternMatches(it, probe.methodName) }
                }
                for (probe in probesToRemove) {
                    classMap.removeProbe(probe.probeIndex)
                }
            }
        }

        for (classId in toRemove) {
            probeMap.removeClassMap(classId)
        }

        if (excludedComposeCount > 0) {
            logger.lifecycle("Omnivore: Auto-excluded $excludedComposeCount pure Compose class(es) from unit test coverage")
        }
    }

    private fun patternMatches(pattern: String, text: String): Boolean =
        GlobPattern.matches(pattern, text)

    // -- Pretty output --

    /**
     * Aggregate the `.stats` files the agent wrote in each test JVM.
     *
     * Returns null when there are none, which is the case for a purely
     * build-time-instrumented (Android) run.
     */
    private fun readInstrumentationSummary(dataDir: File): InstrumentationStatsIo.Summary? =
        dataDir.walkTopDown()
            .filter { it.isFile && it.extension == InstrumentationStatsIo.EXTENSION }
            .mapNotNull { InstrumentationStatsIo.read(it) }
            .reduceOrNull { acc, next -> acc + next }

    private fun printReport(
        targets: List<TargetCoverage>,
        depGraph: DependencyGraph?,
        reportFormats: List<String>,
        outputDir: File,
        instrumentation: InstrumentationStatsIo.Summary?,
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

        // Instrumentation summary. A class can drop out of coverage for
        // several unrelated reasons, each of which used to be a silent
        // `return null` in the transformer — so print what actually happened
        // rather than leaving the user to infer it from a low percentage.
        if (instrumentation != null) {
            val line = buildString {
                append("  Instrumented: ${instrumentation.instrumented} classes")
                if (instrumentation.totalSkipped > 0) {
                    append(", skipped ${instrumentation.totalSkipped} (${instrumentation.describeSkips()})")
                }
            }
            out.style(Style.Description).text(line).println()

            if (instrumentation.failures.isNotEmpty()) {
                out.style(Style.Failure)
                    .text("  ${instrumentation.failures.size} class(es) failed to instrument:")
                    .println()
                for ((className, message) in instrumentation.failures.entries.take(5)) {
                    out.style(Style.Failure).text("    $className: $message").println()
                }
            }
            if (instrumentation.instrumented == 0) {
                out.style(Style.Failure)
                    .text("  No classes were instrumented — coverage will be empty. Check your includes/excludes.")
                    .println()
            }
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

    private fun fmt(pct: Double): String = String.format(java.util.Locale.ROOT, "%5.1f%%", pct)

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
