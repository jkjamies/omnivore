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

            // Local graph output config
            task.localGraphEnabled.convention(extension.dependencies.localGraph.enabled.orElse(false))
            task.localGraphFormat.convention(
                extension.dependencies.localGraph.format.map { it.name }.orElse("MERMAID")
            )
            if (extension.dependencies.localGraph.outputFile.isPresent) {
                task.localGraphOutputFile.convention(extension.dependencies.localGraph.outputFile)
            }

            // Per-target exclusion patterns
            task.unitTestExcludes.convention(extension.unitTests.excludes)
            task.instrumentedTestExcludes.convention(extension.instrumentedTests.excludes)

            // Advanced exclusion options
            task.excludeFiles.convention(extension.excludeFiles)
            task.excludeMethods.convention(extension.excludeMethods)
            task.excludeAnnotations.convention(extension.excludeAnnotations)

            // Depend on all test tasks (this project + subprojects) so coverage data is available
            task.dependsOn(project.tasks.withType(org.gradle.api.tasks.testing.Test::class.java))
            project.subprojects { sub ->
                task.dependsOn(sub.tasks.withType(org.gradle.api.tasks.testing.Test::class.java))
            }
        }

        // Wire instrumented test task dependencies after evaluation
        project.afterEvaluate {
            reportTask.configure { task ->
                if (extension.instrumentedTests.enabled.getOrElse(false)) {
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
        }

        // Resolve dependency graph after ALL projects are evaluated (not just root).
        // Root afterEvaluate fires before subprojects are evaluated, so configurations
        // aren't resolvable yet. gradle.projectsEvaluated fires after everything is done.
        project.gradle.projectsEvaluated {
            if (extension.dependencies.enabled.getOrElse(false)) {
                reportTask.configure { task ->
                    try {
                        val graph = com.jkjamies.omnivore.gradle.configuration.DependencyGraphResolver.resolve(
                            project = project,
                            includeExternal = extension.dependencies.includeExternal.getOrElse(false),
                            includeTestDeps = extension.dependencies.includeTestDeps.getOrElse(false),
                        )
                        task.resolvedDependencyGraph = graph
                        project.logger.lifecycle("Omnivore: Resolved dependency graph: ${graph.modules.size} modules, ${graph.edges.size} edges")
                    } catch (e: Exception) {
                        project.logger.warn("Omnivore: Failed to resolve dependency graph: ${e.message}")
                    }
                }
            }
        }

        // Register the upload task
        project.tasks.register("omnivoreUpload", OmnivoreUploadTask::class.java) { task ->
            task.group = "omnivore"
            task.description = "Upload Omnivore coverage report to the dashboard"
            task.dependsOn(reportTask)

            task.dashboardUrl.convention(extension.dashboard.url)
            // API key resolution: env var > gradle property > DSL
            val apiKeyFromEnv = System.getenv("OMNIVORE_API_KEY")
            val apiKeyFromProp = project.findProperty("omnivore.apiKey") as? String
            val resolvedKey = apiKeyFromEnv ?: apiKeyFromProp
            if (resolvedKey != null) {
                task.authToken.convention(resolvedKey)
            } else {
                task.authToken.convention(extension.dashboard.apiKey)
            }
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
