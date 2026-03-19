package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository

class ToggleTaskUseCase(private val repository: TaskRepository) {

    suspend operator fun invoke(taskId: String): Task? {
        val task = repository.getTask(taskId) ?: return null
        val toggled = task.toggleCompleted()
        repository.updateTask(toggled)
        return toggled
    }
}
