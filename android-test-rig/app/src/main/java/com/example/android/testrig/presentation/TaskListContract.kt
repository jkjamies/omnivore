package com.example.android.testrig.presentation

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.usecase.GetTasksUseCase

/**
 * MVI contract for the task list screen.
 */
object TaskListContract {

    data class State(
        val tasks: List<Task> = emptyList(),
        val filter: GetTasksUseCase.Filter = GetTasksUseCase.Filter.ALL,
        val isLoading: Boolean = false,
        val error: String? = null,
        val searchQuery: String = "",
    ) {
        val filteredTasks: List<Task>
            get() = if (searchQuery.isBlank()) tasks
            else tasks.filter { it.matchesSearch(searchQuery) }

        val activeCount: Int get() = tasks.count { !it.isCompleted }
        val completedCount: Int get() = tasks.count { it.isCompleted }
    }

    sealed class Intent {
        data object LoadTasks : Intent()
        data class ToggleTask(val taskId: String) : Intent()
        data class DeleteTask(val taskId: String) : Intent()
        data class SetFilter(val filter: GetTasksUseCase.Filter) : Intent()
        data class Search(val query: String) : Intent()
        data class AddTask(
            val title: String,
            val description: String = "",
            val priority: Task.Priority = Task.Priority.MEDIUM,
        ) : Intent()
    }

    sealed class Effect {
        data class ShowError(val message: String) : Effect()
        data class TaskAdded(val task: Task) : Effect()
        data object TaskDeleted : Effect()
    }
}
