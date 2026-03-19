package com.example.android.testrig.domain.repository

import com.example.android.testrig.domain.model.Task

interface TaskRepository {
    suspend fun getTasks(): List<Task>
    suspend fun getTask(id: String): Task?
    suspend fun addTask(task: Task)
    suspend fun updateTask(task: Task)
    suspend fun deleteTask(id: String)
    suspend fun getTasksByPriority(priority: Task.Priority): List<Task>
}
