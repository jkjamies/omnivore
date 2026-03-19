package com.example.android.testrig.domain.usecase

import com.example.android.testrig.data.repository.InMemoryTaskRepository
import com.example.android.testrig.domain.model.Task
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldHaveSize
import io.kotest.matchers.shouldBe
import kotlinx.coroutines.test.runTest

class GetTasksUseCaseTest : FunSpec({

    lateinit var repository: InMemoryTaskRepository
    lateinit var useCase: GetTasksUseCase

    beforeEach {
        repository = InMemoryTaskRepository()
        useCase = GetTasksUseCase(repository)
    }

    test("returns all tasks with ALL filter") {
        runTest {
            repository.addTask(Task(id = "1", title = "Task 1"))
            repository.addTask(Task(id = "2", title = "Task 2", isCompleted = true))

            val result = useCase(GetTasksUseCase.Filter.ALL)
            result shouldHaveSize 2
        }
    }

    test("returns only active tasks with ACTIVE filter") {
        runTest {
            repository.addTask(Task(id = "1", title = "Active"))
            repository.addTask(Task(id = "2", title = "Done", isCompleted = true))

            val result = useCase(GetTasksUseCase.Filter.ACTIVE)
            result shouldHaveSize 1
            result[0].title shouldBe "Active"
        }
    }

    // Intentionally not testing COMPLETED filter
})
