package com.example.app

/**
 * A small service with data classes and logic — realistic coverage target.
 */
data class User(
    val id: Int,
    val name: String,
    val email: String,
    val active: Boolean = true,
)

class UserService {

    private val users = mutableListOf<User>()

    fun addUser(user: User): Boolean {
        if (users.any { it.id == user.id }) {
            return false
        }
        users.add(user)
        return true
    }

    fun getUser(id: Int): User? {
        return users.find { it.id == id }
    }

    fun getActiveUsers(): List<User> {
        return users.filter { it.active }
    }

    fun deactivateUser(id: Int): Boolean {
        val index = users.indexOfFirst { it.id == id }
        if (index == -1) return false
        users[index] = users[index].copy(active = false)
        return true
    }

    fun removeUser(id: Int): Boolean {
        return users.removeIf { it.id == id }
    }

    fun getUserCount(): Int = users.size

    fun findByEmail(email: String): User? {
        return users.find { it.email.equals(email, ignoreCase = true) }
    }
}
