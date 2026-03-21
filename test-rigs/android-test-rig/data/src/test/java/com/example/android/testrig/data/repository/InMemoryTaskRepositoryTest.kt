package com.example.android.testrig.data.repository

import com.example.android.testrig.domain.model.Task
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.nulls.shouldBeNull
import io.kotest.matchers.shouldBe
import kotlinx.coroutines.test.runTest

class InMemoryTaskRepositoryTest : FunSpec({

    lateinit var repository: InMemoryTaskRepository

    beforeEach {
        repository = InMemoryTaskRepository()
    }

    test("add and get task") {
        runTest {
            val task = Task(id = "1", title = "Test")
            repository.addTask(task)
            repository.getTask("1") shouldBe task
        }
    }

    test("get nonexistent task returns null") {
        runTest {
            repository.getTask("nonexistent").shouldBeNull()
        }
    }

    test("delete task removes it") {
        runTest {
            repository.addTask(Task(id = "1", title = "Test"))
            repository.deleteTask("1")
            repository.getTask("1").shouldBeNull()
        }
    }

    // Intentionally not testing updateTask or getTasksByPriority
})
