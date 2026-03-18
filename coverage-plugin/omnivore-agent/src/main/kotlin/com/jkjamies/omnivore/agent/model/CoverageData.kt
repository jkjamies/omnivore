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
)

@Serializable
data class LineCoverage(
    val lineNumber: Int,
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
