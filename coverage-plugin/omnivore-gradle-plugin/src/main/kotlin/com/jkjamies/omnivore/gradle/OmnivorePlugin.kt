package com.jkjamies.omnivore.gradle

import com.jkjamies.omnivore.gradle.configuration.InstrumentedTestConfigurator
import com.jkjamies.omnivore.gradle.configuration.UnitTestConfigurator
import com.jkjamies.omnivore.gradle.tasks.OmnivoreReportTask
import com.jkjamies.omnivore.gradle.tasks.OmnivoreUploadTask
import org.gradle.api.Plugin
import org.gradle.api.Project

/**
 * Omnivore Coverage Gradle Plugin.
 *
 * Provides Compose-aware code coverage for Android, Kotlin, and KMP projects.
 *
 * Apply with: `plugins { id("com.jkjamies.omnivore.coverage") }`
 */
class OmnivorePlugin : Plugin<Project> {

    override fun apply(project: Project) {
        val extension = project.extensions.create(
            "omnivore",
            OmnivoreExtension::class.java
        )

        // Set defaults after extension is created
        project.afterEvaluate {
            applyDefaults(project, extension)
        }

        // Configure coverage collection on this project and all subprojects
        UnitTestConfigurator.configure(project, extension)
        InstrumentedTestConfigurator.configure(project, extension)

        project.subprojects { subproject ->
            subproject.afterEvaluate {
                UnitTestConfigurator.configure(subproject, extension)
                InstrumentedTestConfigurator.configure(subproject, extension)
            }
        }

        // Register the report generation task
        val reportTask = project.tasks.register("omnivoreReport", OmnivoreReportTask::class.java) { task ->
            task.group = "omnivore"
            task.description = "Generate Omnivore coverage report"

            // Wire extension config via conventions (lazy, no afterEvaluate needed)
            task.jsonEnabled.convention(extension.reports.json.enabled.orElse(true))
            task.htmlEnabled.convention(extension.reports.html.enabled.orElse(true))
            task.markdownEnabled.convention(extension.reports.markdown.enabled.orElse(false))
            task.projectId.convention(extension.dashboard.projectId.orElse(project.name))
            task.projectName.convention(project.name)

            // Dependency graph config
            task.dependenciesEnabled.convention(extension.dependencies.enabled.orElse(false))
            task.dependenciesIncludeExternal.convention(extension.dependencies.includeExternal.orElse(false))
            task.dependenciesIncludeTestDeps.convention(extension.dependencies.includeTestDeps.orElse(false))

            // Depend on all subproject test tasks so coverage data is available
            project.subprojects { sub ->
                task.dependsOn(sub.tasks.withType(org.gradle.api.tasks.testing.Test::class.java))
            }
        }

        // Register the upload task
        project.tasks.register("omnivoreUpload", OmnivoreUploadTask::class.java) { task ->
            task.group = "omnivore"
            task.description = "Upload Omnivore coverage report to the dashboard"
            task.dependsOn(reportTask)

            task.dashboardUrl.convention(extension.dashboard.url)
            task.authToken.convention(extension.dashboard.token)
        }
    }

    private fun applyDefaults(project: Project, extension: OmnivoreExtension) {
        // Auto-detect Compose and enable filtering
        val hasCompose = project.plugins.hasPlugin("org.jetbrains.compose") ||
            project.plugins.hasPlugin("org.jetbrains.kotlin.plugin.compose")

        if (!extension.composeFilter.enabled.isPresent) {
            extension.composeFilter.enabled.set(hasCompose)
        }

        // Default report formats
        if (!extension.reports.json.enabled.isPresent) {
            extension.reports.json.enabled.set(true)
        }
        if (!extension.reports.html.enabled.isPresent) {
            extension.reports.html.enabled.set(true)
        }
        if (!extension.reports.xml.enabled.isPresent) {
            extension.reports.xml.enabled.set(false)
        }
    }
}
