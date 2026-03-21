package com.example.android.testrig.domain.model

data class Task(
    val id: String,
    val title: String,
    val description: String = "",
    val isCompleted: Boolean = false,
    val priority: Priority = Priority.MEDIUM,
) {
    enum class Priority { LOW, MEDIUM, HIGH }

    fun toggleCompleted(): Task = copy(isCompleted = !isCompleted)

    fun isOverdue(currentTimeMillis: Long, deadlineMillis: Long?): Boolean {
        if (deadlineMillis == null) return false
        return !isCompleted && currentTimeMillis > deadlineMillis
    }

    fun matchesSearch(query: String): Boolean {
        if (query.isBlank()) return true
        val lowerQuery = query.lowercase()
        return title.lowercase().contains(lowerQuery) ||
            description.lowercase().contains(lowerQuery)
    }
}
