package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.model.ModuleEdge
import com.jkjamies.omnivore.agent.model.ModuleNode
import com.jkjamies.omnivore.agent.model.ModuleType
import org.gradle.api.Project
import org.gradle.api.artifacts.ResolvedDependency

/**
 * Resolves the project's dependency graph from Gradle's configuration model.
 *
 * Walks resolved configurations to produce a graph of [ModuleNode]s (project modules
 * and optionally external artifacts) connected by [ModuleEdge]s (dependency relationships
 * labeled with the configuration name like "implementation" or "api").
 */
object DependencyGraphResolver {

    /** Resolvable configurations for production dependencies. */
    private val PROD_CONFIGS = setOf(
        "runtimeClasspath", "compileClasspath",
    )

    /** Resolvable configurations for test dependencies. */
    private val TEST_CONFIGS = setOf(
        "testRuntimeClasspath", "testCompileClasspath",
        "androidTestRuntimeClasspath",
    )

    /** Map resolvable config names to user-friendly labels. */
    private val CONFIG_LABELS = mapOf(
        "runtimeClasspath" to "implementation",
        "compileClasspath" to "implementation",
        "testRuntimeClasspath" to "test",
        "testCompileClasspath" to "test",
        "androidTestRuntimeClasspath" to "test",
    )

    fun resolve(
        project: Project,
        includeExternal: Boolean = false,
        includeTestDeps: Boolean = false,
    ): DependencyGraph {
        val nodes = mutableMapOf<String, ModuleNode>()
        val edges = mutableSetOf<ModuleEdge>()

        // Add the root project module
        val rootId = project.path.ifEmpty { ":" }
        nodes[rootId] = ModuleNode(
            id = rootId,
            name = project.name,
            type = ModuleType.INTERNAL,
        )

        // Collect group IDs that represent internal project modules
        val internalGroupIds = buildSet {
            add(project.group.toString())
            project.rootProject.allprojects.forEach { add(it.group.toString()) }
        }

        val targetConfigs = buildSet {
            addAll(PROD_CONFIGS)
            if (includeTestDeps) addAll(TEST_CONFIGS)
        }

        // Walk configurations for this project and all subprojects
        val projectsToScan = buildList {
            add(project)
            addAll(project.subprojects)
        }

        for (scanProject in projectsToScan) {
            val scanId = scanProject.path.ifEmpty { ":" }
            // Ensure subproject nodes exist
            if (scanId !in nodes) {
                nodes[scanId] = ModuleNode(
                    id = scanId,
                    name = scanProject.name,
                    type = ModuleType.INTERNAL,
                )
            }

            for (config in scanProject.configurations.toList()) {
                val configName = config.name
                // Match exact names (JVM) or variant-prefixed names (Android: debugRuntimeClasspath, etc.)
                val isTarget = configName in targetConfigs || targetConfigs.any { base ->
                    configName.endsWith(base.replaceFirstChar { it.uppercaseChar() })
                }
                if (!isTarget) continue
                if (!config.isCanBeResolved) continue

                val resolved = try {
                    config.resolvedConfiguration
                } catch (e: Exception) {
                    scanProject.logger.info("Omnivore: Failed to resolve config ${scanProject.path}:${configName}: ${e.message}")
                    continue
                }

                for (dep in resolved.firstLevelModuleDependencies) {
                    walkDependency(
                        dep = dep,
                        parentId = scanId,
                        configLabel = simplifyConfigName(configName),
                        nodes = nodes,
                        edges = edges,
                        includeExternal = includeExternal,
                        visited = mutableSetOf(),
                        internalGroupIds = internalGroupIds,
                    )
                }
            }
        }

        // Remove root project node if it's just a container (has subprojects, no edges)
        if (project.subprojects.isNotEmpty()) {
            val hasEdges = edges.any { it.from == rootId || it.to == rootId }
            if (!hasEdges) {
                nodes.remove(rootId)
            }
        }

        // Deduplicate edges: keep one edge per (from, to) pair, preferring
        // "implementation" > "test" > "transitive" labels. Remove self-edges.
        val configPriority = mapOf("implementation" to 0, "api" to 0, "test" to 1, "transitive" to 2)
        val deduped = edges
            .filter { it.from != it.to }
            .groupBy { it.from to it.to }
            .map { (_, group) ->
                group.minByOrNull { configPriority[it.configuration] ?: 99 } ?: group.first()
            }

        return DependencyGraph(
            modules = nodes.values.toList(),
            edges = deduped,
        )
    }

    private fun walkDependency(
        dep: ResolvedDependency,
        parentId: String,
        configLabel: String,
        nodes: MutableMap<String, ModuleNode>,
        edges: MutableSet<ModuleEdge>,
        includeExternal: Boolean,
        visited: MutableSet<String>,
        internalGroupIds: Set<String>,
    ) {
        val idString = dep.module.id.toString()
        val isInternal = idString.startsWith("project ") ||
            dep.moduleGroup in internalGroupIds ||
            dep.moduleGroup.isEmpty()
        val nodeId = if (isInternal) {
            // Project dependency — use module name prefixed with ":"
            ":${dep.moduleName}"
        } else {
            // External dependency — use GAV coordinates
            "${dep.moduleGroup}:${dep.moduleName}:${dep.moduleVersion}"
        }

        if (!isInternal && !includeExternal) return
        if (!visited.add(nodeId)) return // Cycle protection

        nodes.getOrPut(nodeId) {
            if (isInternal) {
                ModuleNode(
                    id = nodeId,
                    name = dep.moduleName,
                    type = ModuleType.INTERNAL,
                )
            } else {
                ModuleNode(
                    id = nodeId,
                    name = dep.moduleName,
                    type = ModuleType.EXTERNAL,
                    group = dep.moduleGroup,
                    version = dep.moduleVersion,
                )
            }
        }

        edges.add(ModuleEdge(from = parentId, to = nodeId, configuration = configLabel))

        // Walk transitive dependencies
        for (child in dep.children) {
            walkDependency(child, nodeId, "transitive", nodes, edges, includeExternal, visited, internalGroupIds)
        }
    }

    private fun simplifyConfigName(name: String): String {
        CONFIG_LABELS[name]?.let { return it }
        // Handle variant-prefixed names like debugRuntimeClasspath → implementation
        for ((base, label) in CONFIG_LABELS) {
            if (name.endsWith(base.replaceFirstChar { it.uppercaseChar() })) {
                return label
            }
        }
        return "implementation"
    }
}
