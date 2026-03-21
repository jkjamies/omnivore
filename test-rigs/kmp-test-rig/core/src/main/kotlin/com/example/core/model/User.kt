package com.example.core.model

data class User(
    val id: Int,
    val name: String,
    val email: String,
    val active: Boolean = true,
) {
    fun matchesSearch(query: String): Boolean {
        if (query.isBlank()) return true
        val lowerQuery = query.lowercase()
        return name.lowercase().contains(lowerQuery) ||
            email.lowercase().contains(lowerQuery)
    }
}
