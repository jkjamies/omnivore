package com.jkjamies.omnivore.agent.reporter

import com.jkjamies.omnivore.agent.model.CoverageSummary
import com.jkjamies.omnivore.agent.model.FileCoverage
import com.jkjamies.omnivore.agent.model.LineCoverage
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType

/**
 * Analyzes execution data and probe maps to produce per-file coverage data.
 *
 * Correlates:
 * - Probe map: which probe index maps to which source file + line number
 * - Execution data: which probes were hit during test execution
 *
 * Result: per-file, per-line coverage with hit counts.
 */
object CoverageAnalyzer {

    data class AnalysisResult(
        val files: List<FileCoverage>,
        val summary: CoverageSummary,
    )

    /**
     * Analyze coverage by correlating execution data with probe maps.
     */
    fun analyze(executionData: ExecutionDataStore, probeMap: ProbeMap): AnalysisResult {
        val fileLines = mutableMapOf<String, MutableMap<Int, LineInfo>>()
        // Branch totals are accumulated per file, then summed — the overall rate
        // must be covered-edges / total-edges across the whole run, never an
        // average of per-file rates.
        val fileBranches = mutableMapOf<String, BranchTally>()

        for (classMap in probeMap.getAllClassMaps()) {
            val probeData = executionData.getData(classMap.classId) ?: continue
            val probes = probeData.probes

            // Determine the file path from the class name + source file
            val filePath = resolveFilePath(classMap.className, classMap.sourceFile)

            val lines = fileLines.getOrPut(filePath) { mutableMapOf() }
            val branches = fileBranches.getOrPut(filePath) { BranchTally() }

            for (probeEntry in classMap.getProbes()) {
                if (probeEntry.lineNumber <= 0) continue

                val isHit = probeEntry.probeIndex < probes.size && probes[probeEntry.probeIndex]

                when (probeEntry.type) {
                    ProbeType.LINE -> {
                        val existing = lines[probeEntry.lineNumber]
                        if (existing == null) {
                            lines[probeEntry.lineNumber] = LineInfo(
                                hitCount = if (isHit) 1L else 0L,
                                branchCount = 0,
                                branchesCovered = 0,
                            )
                        } else if (isHit && existing.hitCount == 0L) {
                            lines[probeEntry.lineNumber] = existing.copy(hitCount = 1L)
                        }
                    }

                    ProbeType.BRANCH -> {
                        // Each branch probe is one control-flow edge. Covered
                        // means that edge was actually taken, not merely that
                        // the condition was reached.
                        branches.total++
                        if (isHit) branches.covered++

                        // Attribute the edge to the line holding the decision.
                        // A branch probe can precede its line probe (a loop
                        // condition at the bottom of the body), so create the
                        // line entry if it isn't there yet rather than dropping
                        // the branch on the floor as the old code did.
                        val existing = lines[probeEntry.lineNumber]
                            ?: LineInfo(hitCount = 0L, branchCount = 0, branchesCovered = 0)
                        lines[probeEntry.lineNumber] = existing.copy(
                            branchCount = existing.branchCount + 1,
                            branchesCovered = existing.branchesCovered + if (isHit) 1 else 0,
                        )
                    }
                }
            }
        }

        // Build FileCoverage from the collected data
        val fileCoverages = fileLines.map { (path, lines) ->
            val sortedLines = lines.entries.sortedBy { it.key }.map { (lineNum, info) ->
                LineCoverage(lineNumber = lineNum, hitCount = info.hitCount)
            }

            val totalLines = sortedLines.size.toLong()
            val coveredLines = sortedLines.count { it.hitCount > 0 }.toLong()
            val lineRate = if (totalLines > 0) coveredLines.toDouble() / totalLines else 0.0

            val tally = fileBranches[path] ?: BranchTally()
            // A file with no branches is reported as 0.0, not 1.0. Claiming
            // "100% branch coverage" for a file that has no branches to cover
            // is meaningless on its own and actively harmful once aggregated —
            // it used to pull directory and project averages upward.
            // Consumers should weight by branchesTotal, which is 0 here.
            val branchRate = if (tally.total > 0) tally.covered.toDouble() / tally.total else 0.0

            FileCoverage(
                path = path,
                lineRate = lineRate,
                branchRate = branchRate,
                lines = sortedLines,
                branchesCovered = tally.covered.toLong(),
                branchesTotal = tally.total.toLong(),
            )
        }.sortedBy { it.path }

        val totalLines = fileCoverages.sumOf { it.lines.size.toLong() }
        val coveredLines = fileCoverages.sumOf { fc -> fc.lines.count { it.hitCount > 0 }.toLong() }
        val totalBranches = fileCoverages.sumOf { it.branchesTotal }
        val coveredBranches = fileCoverages.sumOf { it.branchesCovered }
        val overallLineRate = if (totalLines > 0) coveredLines.toDouble() / totalLines else 0.0
        val overallBranchRate = if (totalBranches > 0) coveredBranches.toDouble() / totalBranches else 0.0

        return AnalysisResult(
            files = fileCoverages,
            summary = CoverageSummary(
                lineRate = overallLineRate,
                branchRate = overallBranchRate,
                linesCovered = coveredLines,
                linesTotal = totalLines,
                branchesCovered = coveredBranches,
                branchesTotal = totalBranches,
            )
        )
    }

    private class BranchTally(var covered: Int = 0, var total: Int = 0)

    /**
     * Resolve a source file path from class name and optional source file name.
     * Converts "com/example/MyClass" + "MyClass.kt" -> "com/example/MyClass.kt"
     */
    private fun resolveFilePath(className: String, sourceFile: String?): String {
        if (sourceFile != null) {
            val packagePath = className.substringBeforeLast('/', "")
            return if (packagePath.isEmpty()) sourceFile else "$packagePath/$sourceFile"
        }
        // Fallback: use class name with .kt extension
        return "${className.substringBefore('$')}.kt"
    }

    private data class LineInfo(
        val hitCount: Long,
        val branchCount: Int,
        val branchesCovered: Int,
    )
}
