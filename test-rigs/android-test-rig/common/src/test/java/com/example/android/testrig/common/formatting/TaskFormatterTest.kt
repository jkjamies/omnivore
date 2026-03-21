package com.example.android.testrig.common.formatting

import com.example.android.testrig.domain.model.Task
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe
import io.kotest.matchers.string.shouldContain

class TaskFormatterTest : FunSpec({

    test("formatTaskLine for incomplete task") {
        val task = Task(id = "1", title = "Buy milk", priority = Task.Priority.MEDIUM)
        val result = TaskFormatter.formatTaskLine(task)
        result shouldBe "[ ] [MEDIUM] Buy milk"
    }

    test("formatTaskLine for completed task") {
        val task = Task(id = "1", title = "Done", isCompleted = true)
        val result = TaskFormatter.formatTaskLine(task)
        result shouldContain "[x]"
    }

    test("formatTaskList with empty list") {
        TaskFormatter.formatTaskList(emptyList()) shouldBe "No tasks."
    }

    test("formatSummary with tasks") {
        val tasks = listOf(
            Task(id = "1", title = "A"),
            Task(id = "2", title = "B", isCompleted = true),
        )
        val result = TaskFormatter.formatSummary(tasks)
        result shouldContain "2 total"
        result shouldContain "50.0%"
    }

    test("truncateTitle leaves short titles unchanged") {
        TaskFormatter.truncateTitle("Short") shouldBe "Short"
    }

    test("truncateTitle truncates long titles with ellipsis") {
        val long = "A".repeat(50)
        val result = TaskFormatter.truncateTitle(long, maxLength = 20)
        result.length shouldBe 20
        result shouldContain "..."
    }

    // Intentionally not testing: formatPriorityBreakdown, formatSummary empty, formatTaskList multi
})
