package com.jkjamies.omnivore.agent.model

import kotlinx.serialization.Serializable

/**
 * Omnivore coverage report data model.
 * This is the JSON schema shared between the Gradle plugin and the Rust dashboard.
 */
@Serializable
data class OmnivoreReport(
    val version: String = "0.1.0",
    val format: String = "omnivore",
    val project: ProjectInfo,
    val coverage: CoverageSummary,
    val files: List<FileCoverage>,
    val dependencies: DependencyGraph? = null,
)

@Serializable
data class ProjectInfo(
    val id: String,
    val name: String,
    val commitSha: String? = null,
    val branch: String? = null,
    val target: CoverageTarget,
)

@Serializable
enum class CoverageTarget {
    JVM_UNIT,
    ANDROID_INSTRUMENTED,
    IOS_UNIT,
    KOTLIN_NATIVE,
    COMPOSITE,
}

@Serializable
data class CoverageSummary(
    val lineRate: Double,
    val branchRate: Double,
    val linesCovered: Long,
    val linesTotal: Long,
    val branchesCovered: Long,
    val branchesTotal: Long,
)

@Serializable
data class FileCoverage(
    val path: String,
    val lineRate: Double,
    val branchRate: Double,
    val lines: List<LineCoverage>,
    /**
     * Branch edges covered and total for this file.
     *
     * Present so consumers can aggregate branch coverage *correctly*. Rolling a
     * directory or project rate up from per-file `branchRate` values gives an
     * unweighted mean, in which a 3-branch file counts as much as a 300-branch
     * one. With these counts a consumer can sum and divide, which is the only
     * aggregation that means anything.
     *
     * Default 0 so reports written by older plugin versions still deserialize.
     */
    val branchesCovered: Long = 0,
    val branchesTotal: Long = 0,
)

@Serializable
data class LineCoverage(
    val lineNumber: Int,
    /**
     * Times this line was executed, where the producing tool tracks that.
     *
     * The Omnivore agent uses boolean probes, so its values are only ever 0 or
     * 1 — a probe records *that* a line ran, not how often. Counting would mean
     * a read-modify-write on every probe, which is both slower and lossy under
     * concurrency without atomics; JaCoCo makes the same tradeoff. Formats that
     * do carry real counts (JaCoCo XML's `ci`) keep them, so consumers must
     * treat this as "0 = uncovered, ≥1 = covered" and only surface an exact
     * count when it is greater than 1.
     */
    val hitCount: Long,
)

// -- Dependency Graph --

@Serializable
data class DependencyGraph(
    val modules: List<ModuleNode>,
    val edges: List<ModuleEdge>,
)

@Serializable
data class ModuleNode(
    val id: String,
    val name: String,
    val type: ModuleType,
    val group: String? = null,
    val version: String? = null,
)

@Serializable
enum class ModuleType {
    INTERNAL,
    EXTERNAL,
}

@Serializable
data class ModuleEdge(
    val from: String,
    val to: String,
    val configuration: String,
)
