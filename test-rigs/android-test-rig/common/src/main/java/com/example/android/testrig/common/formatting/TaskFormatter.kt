package com.example.android.testrig.common.formatting

import com.example.android.testrig.domain.model.Task

/**
 * Formatting utilities for task display.
 */
object TaskFormatter {

    fun formatTaskLine(task: Task): String {
        val status = if (task.isCompleted) "[x]" else "[ ]"
        val priority = task.priority.name
        return "$status [$priority] ${task.title}"
    }

    fun formatTaskList(tasks: List<Task>): String {
        if (tasks.isEmpty()) return "No tasks."
        return tasks.joinToString("\n") { formatTaskLine(it) }
    }

    fun formatSummary(tasks: List<Task>): String {
        val total = tasks.size
        if (total == 0) return "No tasks tracked."
        val completed = tasks.count { it.isCompleted }
        val pending = total - completed
        val pct = (completed.toDouble() / total) * 100
        return "Tasks: $total total, $completed completed, $pending pending (${String.format("%.1f", pct)}% done)"
    }

    fun formatPriorityBreakdown(tasks: List<Task>): String {
        if (tasks.isEmpty()) return "No tasks to analyze."
        val total = tasks.size
        return Task.Priority.entries.joinToString("\n") { priority ->
            val count = tasks.count { it.priority == priority }
            val pct = (count.toDouble() / total) * 100
            val bar = "#".repeat((pct / 5).toInt())
            "  ${priority.name.padStart(6)}: $bar $count (${String.format("%.0f", pct)}%)"
        }
    }

    fun truncateTitle(title: String, maxLength: Int = 40): String {
        return if (title.length <= maxLength) title
        else title.take(maxLength - 3) + "..."
    }
}
