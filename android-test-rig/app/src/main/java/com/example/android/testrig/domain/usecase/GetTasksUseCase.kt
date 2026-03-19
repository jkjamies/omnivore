package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository

class GetTasksUseCase(private val repository: TaskRepository) {

    suspend operator fun invoke(filter: Filter = Filter.ALL): List<Task> {
        val tasks = repository.getTasks()
        return when (filter) {
            Filter.ALL -> tasks
            Filter.ACTIVE -> tasks.filter { !it.isCompleted }
            Filter.COMPLETED -> tasks.filter { it.isCompleted }
        }
    }

    enum class Filter { ALL, ACTIVE, COMPLETED }
}
