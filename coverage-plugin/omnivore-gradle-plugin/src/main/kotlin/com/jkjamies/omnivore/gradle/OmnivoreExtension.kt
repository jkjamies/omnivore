package com.jkjamies.omnivore.gradle

import org.gradle.api.Action
import org.gradle.api.model.ObjectFactory
import org.gradle.api.provider.ListProperty
import org.gradle.api.provider.Property
import javax.inject.Inject

/**
 * DSL extension for configuring Omnivore coverage.
 *
 * Usage in build.gradle.kts:
 * ```
 * omnivore {
 *     composeFilter {
 *         enabled = true
 *     }
 *     instrumentedTests {
 *         enabled = true
 *     }
 *     reports {
 *         json.enabled = true
 *         html.enabled = true
 *     }
 *     dashboard {
 *         url = "https://omnivore.example.com"
 *         token = providers.environmentVariable("OMNIVORE_TOKEN")
 *         projectId = "my-project"
 *     }
 * }
 * ```
 */
abstract class OmnivoreExtension @Inject constructor(
    objects: ObjectFactory,
) {
    /** Package patterns to include in coverage (empty = all) */
    val includes: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Package patterns to exclude from coverage */
    val excludes: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Compose bytecode filter configuration */
    val composeFilter: ComposeFilterConfig = objects.newInstance(ComposeFilterConfig::class.java)

    /** Instrumented test configuration */
    val instrumentedTests: InstrumentedTestConfig = objects.newInstance(InstrumentedTestConfig::class.java)

    /** Report output configuration */
    val reports: ReportsConfig = objects.newInstance(ReportsConfig::class.java)

    /** Dashboard upload configuration */
    val dashboard: DashboardConfig = objects.newInstance(DashboardConfig::class.java)

    /** Dependency graph configuration */
    val dependencies: DependencyGraphConfig = objects.newInstance(DependencyGraphConfig::class.java)

    fun composeFilter(action: Action<ComposeFilterConfig>) = action.execute(composeFilter)
    fun instrumentedTests(action: Action<InstrumentedTestConfig>) = action.execute(instrumentedTests)
    fun reports(action: Action<ReportsConfig>) = action.execute(reports)
    fun dashboard(action: Action<DashboardConfig>) = action.execute(dashboard)
    fun dependencies(action: Action<DependencyGraphConfig>) = action.execute(dependencies)
}

abstract class ComposeFilterConfig {
    /** Enable Compose bytecode filtering. Auto-detected if Compose plugin is applied. */
    abstract val enabled: Property<Boolean>

    /** Additional class patterns to exclude (beyond built-in Compose patterns) */
    abstract val additionalExcludePatterns: ListProperty<String>
}

abstract class InstrumentedTestConfig {
    /** Enable coverage collection for Android instrumented tests */
    abstract val enabled: Property<Boolean>
}

abstract class ReportsConfig @Inject constructor(objects: ObjectFactory) {
    val json: ReportFormatConfig = objects.newInstance(ReportFormatConfig::class.java)
    val html: ReportFormatConfig = objects.newInstance(ReportFormatConfig::class.java)
    val markdown: ReportFormatConfig = objects.newInstance(ReportFormatConfig::class.java)
    val xml: ReportFormatConfig = objects.newInstance(ReportFormatConfig::class.java)

    fun json(action: Action<ReportFormatConfig>) = action.execute(json)
    fun html(action: Action<ReportFormatConfig>) = action.execute(html)
    fun markdown(action: Action<ReportFormatConfig>) = action.execute(markdown)
    fun xml(action: Action<ReportFormatConfig>) = action.execute(xml)
}

abstract class ReportFormatConfig {
    abstract val enabled: Property<Boolean>
}

abstract class DependencyGraphConfig {
    /** Enable dependency graph collection in reports */
    abstract val enabled: Property<Boolean>

    /** Include external (third-party) dependencies, not just project modules */
    abstract val includeExternal: Property<Boolean>

    /** Include test-scoped dependencies */
    abstract val includeTestDeps: Property<Boolean>
}

abstract class DashboardConfig {
    /** Omnivore dashboard server URL */
    abstract val url: Property<String>

    /** Authentication token */
    abstract val token: Property<String>

    /** Project identifier on the dashboard */
    abstract val projectId: Property<String>
}
