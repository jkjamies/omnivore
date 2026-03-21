package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.repository.TaskRepository

class DeleteTaskUseCase(private val repository: TaskRepository) {

    suspend operator fun invoke(taskId: String): Boolean {
        val task = repository.getTask(taskId) ?: return false
        repository.deleteTask(taskId)
        return true
    }
}
