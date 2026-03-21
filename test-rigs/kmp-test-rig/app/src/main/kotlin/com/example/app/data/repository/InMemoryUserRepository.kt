package com.example.app.data.repository

import com.example.core.model.User
import com.example.core.repository.UserRepository

class InMemoryUserRepository : UserRepository {

    private val users = mutableMapOf<Int, User>()

    override fun getAll(): List<User> = users.values.toList()

    override fun getById(id: Int): User? = users[id]

    override fun add(user: User) {
        users[user.id] = user
    }

    override fun update(user: User) {
        users[user.id] = user
    }

    override fun remove(id: Int): Boolean = users.remove(id) != null

    override fun findByEmail(email: String): User? {
        return users.values.find { it.email.equals(email, ignoreCase = true) }
    }
}
