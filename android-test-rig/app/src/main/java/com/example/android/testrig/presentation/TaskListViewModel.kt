package com.example.android.testrig.presentation

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.example.android.testrig.domain.usecase.AddTaskUseCase
import com.example.android.testrig.domain.usecase.DeleteTaskUseCase
import com.example.android.testrig.domain.usecase.GetTasksUseCase
import com.example.android.testrig.domain.usecase.ToggleTaskUseCase
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class TaskListViewModel(
    private val getTasksUseCase: GetTasksUseCase,
    private val toggleTaskUseCase: ToggleTaskUseCase,
    private val addTaskUseCase: AddTaskUseCase,
    private val deleteTaskUseCase: DeleteTaskUseCase,
) : ViewModel() {

    private val _state = MutableStateFlow(TaskListContract.State())
    val state: StateFlow<TaskListContract.State> = _state.asStateFlow()

    private val _effects = MutableSharedFlow<TaskListContract.Effect>()
    val effects: SharedFlow<TaskListContract.Effect> = _effects.asSharedFlow()

    fun processIntent(intent: TaskListContract.Intent) {
        when (intent) {
            is TaskListContract.Intent.LoadTasks -> loadTasks()
            is TaskListContract.Intent.ToggleTask -> toggleTask(intent.taskId)
            is TaskListContract.Intent.DeleteTask -> deleteTask(intent.taskId)
            is TaskListContract.Intent.SetFilter -> setFilter(intent.filter)
            is TaskListContract.Intent.Search -> search(intent.query)
            is TaskListContract.Intent.AddTask -> addTask(intent.title, intent.description, intent.priority)
        }
    }

    private fun loadTasks() {
        viewModelScope.launch {
            _state.value = _state.value.copy(isLoading = true, error = null)
            try {
                val tasks = getTasksUseCase(_state.value.filter)
                _state.value = _state.value.copy(tasks = tasks, isLoading = false)
            } catch (e: Exception) {
                _state.value = _state.value.copy(isLoading = false, error = e.message)
                _effects.emit(TaskListContract.Effect.ShowError(e.message ?: "Unknown error"))
            }
        }
    }

    private fun toggleTask(taskId: String) {
        viewModelScope.launch {
            try {
                toggleTaskUseCase(taskId)
                loadTasks()
            } catch (e: Exception) {
                _effects.emit(TaskListContract.Effect.ShowError("Failed to toggle task"))
            }
        }
    }

    private fun deleteTask(taskId: String) {
        viewModelScope.launch {
            try {
                val deleted = deleteTaskUseCase(taskId)
                if (deleted) {
                    _effects.emit(TaskListContract.Effect.TaskDeleted)
                    loadTasks()
                } else {
                    _effects.emit(TaskListContract.Effect.ShowError("Task not found"))
                }
            } catch (e: Exception) {
                _effects.emit(TaskListContract.Effect.ShowError("Failed to delete task"))
            }
        }
    }

    private fun setFilter(filter: GetTasksUseCase.Filter) {
        _state.value = _state.value.copy(filter = filter)
        loadTasks()
    }

    private fun search(query: String) {
        _state.value = _state.value.copy(searchQuery = query)
    }

    private fun addTask(title: String, description: String, priority: com.example.android.testrig.domain.model.Task.Priority) {
        viewModelScope.launch {
            when (val result = addTaskUseCase(title, description, priority)) {
                is AddTaskUseCase.Result.Success -> {
                    _effects.emit(TaskListContract.Effect.TaskAdded(result.task))
                    loadTasks()
                }
                is AddTaskUseCase.Result.ValidationError -> {
                    _effects.emit(TaskListContract.Effect.ShowError(result.message))
                }
            }
        }
    }
}
