package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe
import io.kotest.matchers.types.shouldBeInstanceOf
import kotlinx.coroutines.test.runTest

class AddTaskUseCaseTest : FunSpec({

    test("adds task with valid title") {
        runTest {
            val repository = FakeAddTaskRepository()
            val useCase = AddTaskUseCase(repository)

            val result = useCase("Buy groceries", "Milk and eggs", Task.Priority.LOW)
            result.shouldBeInstanceOf<AddTaskUseCase.Result.Success>()
            result.task.title shouldBe "Buy groceries"
            result.task.priority shouldBe Task.Priority.LOW
        }
    }

    test("returns error for blank title") {
        runTest {
            val repository = FakeAddTaskRepository()
            val useCase = AddTaskUseCase(repository)

            val result = useCase("", "desc", Task.Priority.MEDIUM)
            result.shouldBeInstanceOf<AddTaskUseCase.Result.ValidationError>()
            result.message shouldBe "Title cannot be blank"
        }
    }

    // Intentionally not testing title length > 200
})

internal class FakeAddTaskRepository : TaskRepository {
    private val tasks = mutableMapOf<String, Task>()
    override suspend fun getTasks() = tasks.values.toList()
    override suspend fun getTask(id: String) = tasks[id]
    override suspend fun addTask(task: Task) { tasks[task.id] = task }
    override suspend fun updateTask(task: Task) { tasks[task.id] = task }
    override suspend fun deleteTask(id: String) { tasks.remove(id) }
    override suspend fun getTasksByPriority(priority: Task.Priority) =
        tasks.values.filter { it.priority == priority }
}
