package com.jkjamies.omnivore.agent.reporter

import com.jkjamies.omnivore.agent.model.CoverageTarget
import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.model.OmnivoreReport
import com.jkjamies.omnivore.agent.model.ProjectInfo
import kotlinx.serialization.json.Json
import java.io.File

/**
 * Writes coverage reports in the Omnivore JSON format.
 * This is the primary format consumed by the Omnivore Dashboard.
 */
object JsonReportWriter {

    private val json = Json {
        prettyPrint = true
        encodeDefaults = true
    }

    fun write(
        outputFile: File,
        analysisResult: CoverageAnalyzer.AnalysisResult,
        projectId: String,
        projectName: String,
        target: CoverageTarget = CoverageTarget.JVM_UNIT,
        commitSha: String? = null,
        branch: String? = null,
        dependencyGraph: DependencyGraph? = null,
    ) {
        val report = OmnivoreReport(
            project = ProjectInfo(
                id = projectId,
                name = projectName,
                commitSha = commitSha,
                branch = branch,
                target = target,
            ),
            coverage = analysisResult.summary,
            files = analysisResult.files,
            dependencies = dependencyGraph,
        )

        outputFile.parentFile?.mkdirs()
        outputFile.writeText(json.encodeToString(OmnivoreReport.serializer(), report))
    }
}
