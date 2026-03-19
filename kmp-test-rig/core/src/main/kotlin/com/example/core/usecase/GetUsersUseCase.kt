package com.example.core.usecase

import com.example.core.model.User
import com.example.core.repository.UserRepository

class GetUsersUseCase(private val repository: UserRepository) {

    operator fun invoke(filter: Filter = Filter.ALL): List<User> {
        val users = repository.getAll()
        return when (filter) {
            Filter.ALL -> users
            Filter.ACTIVE -> users.filter { it.active }
            Filter.INACTIVE -> users.filter { !it.active }
        }
    }

    enum class Filter { ALL, ACTIVE, INACTIVE }
}
