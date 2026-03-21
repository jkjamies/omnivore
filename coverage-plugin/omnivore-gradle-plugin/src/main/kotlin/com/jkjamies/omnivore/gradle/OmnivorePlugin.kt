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

        // Configure coverage collection on this project and all subprojects.
        // The configurators split work between eager (configuration phase) and
        // deferred (afterEvaluate) to satisfy AGP's requirement that dependencies
        // are added before configurations are resolved.
        UnitTestConfigurator.configure(project, extension)
        InstrumentedTestConfigurator.configure(project, extension)

        project.subprojects { subproject ->
            // Eager: add runtime dependency & AGP transform during configuration phase
            InstrumentedTestConfigurator.configure(subproject, extension)

            subproject.afterEvaluate {
                UnitTestConfigurator.configure(subproject, extension)
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

            // Depend on all test tasks (this project + subprojects) so coverage data is available
            task.dependsOn(project.tasks.withType(org.gradle.api.tasks.testing.Test::class.java))
            project.subprojects { sub ->
                task.dependsOn(sub.tasks.withType(org.gradle.api.tasks.testing.Test::class.java))
            }
        }

        // Wire instrumented test task dependencies after evaluation
        project.afterEvaluate {
            reportTask.configure { task ->
                val allProjects = listOf(project) + project.subprojects
                for (p in allProjects) {
                    // Depend on connected Android test tasks so omnivoreReport triggers them
                    p.tasks.names.filter {
                        it.startsWith("connected") && it.endsWith("AndroidTest")
                    }.forEach { name ->
                        task.dependsOn(p.tasks.named(name))
                    }
                    // Depend on build-time probe map task (writes .probes from AGP transform)
                    p.tasks.findByName("omnivoreWriteBuildProbeMap")?.let { task.dependsOn(it) }
                }
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
