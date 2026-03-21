package com.example.android.testrig.domain.usecase

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository

class AddTaskUseCase(private val repository: TaskRepository) {

    sealed class Result {
        data class Success(val task: Task) : Result()
        data class ValidationError(val message: String) : Result()
    }

    suspend operator fun invoke(title: String, description: String, priority: Task.Priority): Result {
        if (title.isBlank()) {
            return Result.ValidationError("Title cannot be blank")
        }
        if (title.length > 200) {
            return Result.ValidationError("Title too long (max 200 characters)")
        }

        val task = Task(
            id = generateId(),
            title = title.trim(),
            description = description.trim(),
            priority = priority,
        )
        repository.addTask(task)
        return Result.Success(task)
    }

    private fun generateId(): String = java.util.UUID.randomUUID().toString()
}
