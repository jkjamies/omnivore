package com.example.android.testrig.data.repository

import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.repository.TaskRepository

class InMemoryTaskRepository : TaskRepository {

    private val tasks = mutableMapOf<String, Task>()

    override suspend fun getTasks(): List<Task> = tasks.values.toList()

    override suspend fun getTask(id: String): Task? = tasks[id]

    override suspend fun addTask(task: Task) {
        tasks[task.id] = task
    }

    override suspend fun updateTask(task: Task) {
        tasks[task.id] = task
    }

    override suspend fun deleteTask(id: String) {
        tasks.remove(id)
    }

    override suspend fun getTasksByPriority(priority: Task.Priority): List<Task> {
        return tasks.values.filter { it.priority == priority }
    }
}
