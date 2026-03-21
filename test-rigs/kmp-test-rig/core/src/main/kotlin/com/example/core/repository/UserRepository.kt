package com.example.core.repository

import com.example.core.model.User

interface UserRepository {
    fun getAll(): List<User>
    fun getById(id: Int): User?
    fun add(user: User)
    fun update(user: User)
    fun remove(id: Int): Boolean
    fun findByEmail(email: String): User?
}
