package com.example.android.testrig.domain.usecase

import com.example.android.testrig.data.repository.InMemoryTaskRepository
import com.example.android.testrig.domain.model.Task
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe
import io.kotest.matchers.types.shouldBeInstanceOf
import kotlinx.coroutines.test.runTest

class AddTaskUseCaseTest : FunSpec({

    lateinit var repository: InMemoryTaskRepository
    lateinit var useCase: AddTaskUseCase

    beforeEach {
        repository = InMemoryTaskRepository()
        useCase = AddTaskUseCase(repository)
    }

    test("adds task with valid title") {
        runTest {
            val result = useCase("Buy groceries", "Milk and eggs", Task.Priority.LOW)

            result.shouldBeInstanceOf<AddTaskUseCase.Result.Success>()
            result.task.title shouldBe "Buy groceries"
            result.task.priority shouldBe Task.Priority.LOW
        }
    }

    test("returns error for blank title") {
        runTest {
            val result = useCase("", "desc", Task.Priority.MEDIUM)

            result.shouldBeInstanceOf<AddTaskUseCase.Result.ValidationError>()
            result.message shouldBe "Title cannot be blank"
        }
    }

    // Intentionally not testing title length > 200
})
