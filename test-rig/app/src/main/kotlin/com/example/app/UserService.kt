package com.example.app

import com.example.core.OpResult
import com.example.core.Validation

/**
 * A small service with data classes and logic — realistic coverage target.
 * Uses core module for validation and result types.
 */
data class User(
    val id: Int,
    val name: String,
    val email: String,
    val active: Boolean = true,
)

class UserService {

    private val users = mutableListOf<User>()

    fun addUser(user: User): OpResult<User> {
        if (!Validation.isValidId(user.id)) {
            return OpResult.Failure("Invalid user ID")
        }
        if (!Validation.isValidName(user.name)) {
            return OpResult.Failure("Invalid name")
        }
        if (!Validation.isValidEmail(user.email)) {
            return OpResult.Failure("Invalid email")
        }
        if (users.any { it.id == user.id }) {
            return OpResult.Failure("User already exists")
        }
        val sanitized = user.copy(name = Validation.sanitize(user.name))
        users.add(sanitized)
        return OpResult.Success(sanitized)
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
