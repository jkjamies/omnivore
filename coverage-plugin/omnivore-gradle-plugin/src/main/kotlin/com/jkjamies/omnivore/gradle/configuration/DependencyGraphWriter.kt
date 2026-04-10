package com.jkjamies.omnivore.gradle.configuration

import com.jkjamies.omnivore.agent.model.DependencyGraph
import com.jkjamies.omnivore.agent.model.ModuleType
import com.jkjamies.omnivore.gradle.GraphFormat
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

/**
 * Writes a dependency graph to a local file in DOT, Mermaid, or JSON format.
 */
object DependencyGraphWriter {

    private val json = Json { prettyPrint = true }

    fun write(file: File, graph: DependencyGraph, format: GraphFormat) {
        file.parentFile?.mkdirs()
        val content = when (format) {
            GraphFormat.DOT -> toDot(graph)
            GraphFormat.MERMAID -> toMermaid(graph)
            GraphFormat.JSON -> toJson(graph)
        }
        file.writeText(content)
    }

    fun defaultExtension(format: GraphFormat): String = when (format) {
        GraphFormat.DOT -> "dot"
        GraphFormat.MERMAID -> "md"
        GraphFormat.JSON -> "json"
    }

    private fun toDot(graph: DependencyGraph): String = buildString {
        appendLine("digraph dependencies {")
        appendLine("    rankdir=LR;")
        appendLine("    node [shape=box, style=filled];")
        appendLine()

        for (node in graph.modules) {
            val color = if (node.type == ModuleType.INTERNAL) "#4CAF50" else "#2196F3"
            val label = if (node.type == ModuleType.EXTERNAL && node.version != null) {
                "${node.name}\\n${node.version}"
            } else {
                node.name
            }
            appendLine("    \"${node.id}\" [label=\"$label\", fillcolor=\"$color\", fontcolor=white];")
        }

        appendLine()

        for (edge in graph.edges) {
            val style = when (edge.configuration) {
                "test" -> "dashed"
                "transitive" -> "dotted"
                else -> "solid"
            }
            appendLine("    \"${edge.from}\" -> \"${edge.to}\" [label=\"${edge.configuration}\", style=$style];")
        }

        appendLine("}")
    }

    private fun toMermaid(graph: DependencyGraph): String = buildString {
        appendLine("```mermaid")
        appendLine("graph LR")
        appendLine()

        for (node in graph.modules) {
            val sanitizedId = sanitizeMermaidId(node.id)
            val label = if (node.type == ModuleType.EXTERNAL && node.version != null) {
                "${node.name}<br/>${node.version}"
            } else {
                node.name
            }
            if (node.type == ModuleType.INTERNAL) {
                appendLine("    $sanitizedId[\"$label\"]")
            } else {
                appendLine("    $sanitizedId([\"$label\"])")
            }
        }

        appendLine()

        for (edge in graph.edges) {
            val fromId = sanitizeMermaidId(edge.from)
            val toId = sanitizeMermaidId(edge.to)
            val arrow = when (edge.configuration) {
                "test" -> "-.->"
                "transitive" -> "-..->"
                else -> "-->"
            }
            appendLine("    $fromId $arrow|${edge.configuration}| $toId")
        }

        appendLine()

        // Style internal vs external
        val internalIds = graph.modules.filter { it.type == ModuleType.INTERNAL }.map { sanitizeMermaidId(it.id) }
        val externalIds = graph.modules.filter { it.type == ModuleType.EXTERNAL }.map { sanitizeMermaidId(it.id) }

        if (internalIds.isNotEmpty()) {
            appendLine("    classDef internal fill:#4CAF50,color:white,stroke:#388E3C")
            appendLine("    class ${internalIds.joinToString(",")} internal")
        }
        if (externalIds.isNotEmpty()) {
            appendLine("    classDef external fill:#2196F3,color:white,stroke:#1976D2")
            appendLine("    class ${externalIds.joinToString(",")} external")
        }

        appendLine("```")
    }

    private fun toJson(graph: DependencyGraph): String {
        return json.encodeToString(graph)
    }

    private fun sanitizeMermaidId(id: String): String {
        return id.replace(":", "_").replace(".", "_").replace("-", "_").replace("/", "_")
            .trimStart('_')
    }
}
