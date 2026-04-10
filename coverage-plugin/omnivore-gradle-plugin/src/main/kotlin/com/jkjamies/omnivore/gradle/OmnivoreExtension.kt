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

    /** Package patterns to exclude from coverage (glob or regex:pattern) */
    val excludes: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Annotation class names that exclude a class or method from coverage */
    val excludeAnnotations: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Source file path patterns to exclude from coverage reports */
    val excludeFiles: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Method name patterns to exclude from coverage (glob or regex:pattern) */
    val excludeMethods: ListProperty<String> = objects.listProperty(String::class.java)
        .convention(emptyList())

    /** Path to a file containing exclusion patterns (one per line, # for comments) */
    val excludesFile: Property<java.io.File> = objects.property(java.io.File::class.java)

    /** Compose bytecode filter configuration (always enabled; zero-cost on non-Compose projects) */
    val composeFilter: ComposeFilterConfig = objects.newInstance(ComposeFilterConfig::class.java)

    /** Unit test configuration */
    val unitTests: UnitTestConfig = objects.newInstance(UnitTestConfig::class.java)

    /** Instrumented test configuration */
    val instrumentedTests: InstrumentedTestConfig = objects.newInstance(InstrumentedTestConfig::class.java)

    /** Report output configuration */
    val reports: ReportsConfig = objects.newInstance(ReportsConfig::class.java)

    /** Dashboard upload configuration */
    val dashboard: DashboardConfig = objects.newInstance(DashboardConfig::class.java)

    /** Dependency graph configuration */
    val dependencies: DependencyGraphConfig = objects.newInstance(DependencyGraphConfig::class.java)

    fun composeFilter(action: Action<ComposeFilterConfig>) = action.execute(composeFilter)
    fun unitTests(action: Action<UnitTestConfig>) = action.execute(unitTests)
    fun instrumentedTests(action: Action<InstrumentedTestConfig>) = action.execute(instrumentedTests)
    fun reports(action: Action<ReportsConfig>) = action.execute(reports)
    fun dashboard(action: Action<DashboardConfig>) = action.execute(dashboard)
    fun dependencies(action: Action<DependencyGraphConfig>) = action.execute(dependencies)
}

abstract class ComposeFilterConfig {
    /** Additional class patterns to exclude (beyond built-in Compose patterns) */
    abstract val additionalExcludePatterns: ListProperty<String>
}

abstract class UnitTestConfig {
    /** Package patterns to exclude from unit test coverage (in addition to global excludes) */
    abstract val excludes: ListProperty<String>
}

abstract class InstrumentedTestConfig {
    /** Enable coverage collection for Android instrumented tests */
    abstract val enabled: Property<Boolean>

    /** Package patterns to exclude from instrumented test coverage (in addition to global excludes) */
    abstract val excludes: ListProperty<String>
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

abstract class DependencyGraphConfig @Inject constructor(objects: ObjectFactory) {
    /** Enable dependency graph collection in reports */
    abstract val enabled: Property<Boolean>

    /** Include external (third-party) dependencies, not just project modules */
    abstract val includeExternal: Property<Boolean>

    /** Include test-scoped dependencies */
    abstract val includeTestDeps: Property<Boolean>

    /** Local file output configuration */
    val localGraph: LocalGraphConfig = objects.newInstance(LocalGraphConfig::class.java)

    fun localGraph(action: Action<LocalGraphConfig>) = action.execute(localGraph)
}

/** Output format for the local dependency graph file */
enum class GraphFormat {
    DOT,
    MERMAID,
    JSON,
}

abstract class LocalGraphConfig {
    /** Enable local dependency graph file output */
    abstract val enabled: Property<Boolean>

    /** Output file path (defaults to build/reports/omnivore/dependency-graph.{ext}) */
    abstract val outputFile: Property<java.io.File>

    /** Output format: DOT, MERMAID, or JSON */
    abstract val format: Property<GraphFormat>
}

abstract class DashboardConfig {
    /** Omnivore dashboard server URL */
    abstract val url: Property<String>

    /** API key for authenticated uploads (prefer env var OMNIVORE_API_KEY or gradle property omnivore.apiKey) */
    abstract val apiKey: Property<String>

    /** Project identifier on the dashboard */
    abstract val projectId: Property<String>
}
