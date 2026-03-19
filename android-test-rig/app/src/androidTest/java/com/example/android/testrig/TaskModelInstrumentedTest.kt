package com.example.android.testrig

import androidx.test.ext.junit.runners.AndroidJUnit4
import com.example.android.testrig.domain.model.Task
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests verifying domain model behavior on a real Android device/emulator.
 */
@RunWith(AndroidJUnit4::class)
class TaskModelInstrumentedTest {

    @Test
    fun toggleCompleted_flips_state() {
        val task = Task(id = "1", title = "Test", isCompleted = false)
        val toggled = task.toggleCompleted()
        assertTrue(toggled.isCompleted)
    }

    @Test
    fun toggleCompleted_round_trip() {
        val task = Task(id = "1", title = "Test", isCompleted = false)
        val result = task.toggleCompleted().toggleCompleted()
        assertFalse(result.isCompleted)
    }

    @Test
    fun matchesSearch_finds_in_title() {
        val task = Task(id = "1", title = "Buy groceries", description = "")
        assertTrue(task.matchesSearch("buy"))
        assertTrue(task.matchesSearch("GROCERIES"))
        assertFalse(task.matchesSearch("code"))
    }

    @Test
    fun matchesSearch_empty_query_matches_all() {
        val task = Task(id = "1", title = "Any task")
        assertTrue(task.matchesSearch(""))
        assertTrue(task.matchesSearch("  "))
    }

    @Test
    fun priority_ordering() {
        assertEquals(0, Task.Priority.LOW.ordinal)
        assertEquals(1, Task.Priority.MEDIUM.ordinal)
        assertEquals(2, Task.Priority.HIGH.ordinal)
    }

    // Intentionally not testing: isOverdue, matchesSearch on description
}
