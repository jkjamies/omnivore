package com.example.android.testrig.presentation

import com.example.android.testrig.data.repository.InMemoryTaskRepository
import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.usecase.AddTaskUseCase
import com.example.android.testrig.domain.usecase.DeleteTaskUseCase
import com.example.android.testrig.domain.usecase.GetTasksUseCase
import com.example.android.testrig.domain.usecase.ToggleTaskUseCase
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldHaveSize
import io.kotest.matchers.shouldBe
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain

@OptIn(ExperimentalCoroutinesApi::class)
class TaskListViewModelTest : FunSpec({

    lateinit var repository: InMemoryTaskRepository
    lateinit var viewModel: TaskListViewModel
    val testDispatcher = UnconfinedTestDispatcher()

    beforeEach {
        Dispatchers.setMain(testDispatcher)
        repository = InMemoryTaskRepository()
        viewModel = TaskListViewModel(
            getTasksUseCase = GetTasksUseCase(repository),
            toggleTaskUseCase = ToggleTaskUseCase(repository),
            addTaskUseCase = AddTaskUseCase(repository),
            deleteTaskUseCase = DeleteTaskUseCase(repository),
        )
    }

    afterEach {
        Dispatchers.resetMain()
    }

    test("load tasks updates state") {
        runTest {
            repository.addTask(Task(id = "1", title = "Task 1"))
            repository.addTask(Task(id = "2", title = "Task 2"))

            viewModel.processIntent(TaskListContract.Intent.LoadTasks)

            viewModel.state.value.tasks shouldHaveSize 2
            viewModel.state.value.isLoading shouldBe false
        }
    }

    test("search filters displayed tasks") {
        runTest {
            repository.addTask(Task(id = "1", title = "Buy groceries"))
            repository.addTask(Task(id = "2", title = "Write code"))

            viewModel.processIntent(TaskListContract.Intent.LoadTasks)
            viewModel.processIntent(TaskListContract.Intent.Search("buy"))

            viewModel.state.value.searchQuery shouldBe "buy"
            viewModel.state.value.filteredTasks shouldHaveSize 1
        }
    }

    // Intentionally not testing: ToggleTask, DeleteTask, AddTask intents, SetFilter, effects
})
