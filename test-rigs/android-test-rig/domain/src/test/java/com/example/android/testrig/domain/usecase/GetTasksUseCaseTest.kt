package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldHaveSize
import io.kotest.matchers.shouldBe
import kotlinx.coroutines.test.runTest

class GetTasksUseCaseTest : FunSpec({

    test("returns all tasks with ALL filter") {
        runTest {
            val repository = FakeGetTasksRepository()
            repository.addTask(Task(id = "1", title = "Task 1"))
            repository.addTask(Task(id = "2", title = "Task 2", isCompleted = true))

            val useCase = GetTasksUseCase(repository)
            val result = useCase(GetTasksUseCase.Filter.ALL)
            result shouldHaveSize 2
        }
    }

    test("returns only active tasks with ACTIVE filter") {
        runTest {
            val repository = FakeGetTasksRepository()
            repository.addTask(Task(id = "1", title = "Active"))
            repository.addTask(Task(id = "2", title = "Done", isCompleted = true))

            val useCase = GetTasksUseCase(repository)
            val result = useCase(GetTasksUseCase.Filter.ACTIVE)
            result shouldHaveSize 1
            result[0].title shouldBe "Active"
        }
    }

    // Intentionally not testing COMPLETED filter
})

/** Simple fake for domain-level tests (no dependency on :data module). */
internal class FakeGetTasksRepository : TaskRepository {
    private val tasks = mutableMapOf<String, Task>()
    override suspend fun getTasks() = tasks.values.toList()
    override suspend fun getTask(id: String) = tasks[id]
    override suspend fun addTask(task: Task) { tasks[task.id] = task }
    override suspend fun updateTask(task: Task) { tasks[task.id] = task }
    override suspend fun deleteTask(id: String) { tasks.remove(id) }
    override suspend fun getTasksByPriority(priority: Task.Priority) =
        tasks.values.filter { it.priority == priority }
}
