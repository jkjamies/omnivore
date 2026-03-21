package com.example.core.usecase

import com.example.core.model.OpResult
import com.example.core.model.User
import com.example.core.repository.UserRepository

class DeactivateUserUseCase(private val repository: UserRepository) {

    operator fun invoke(userId: Int): OpResult<User> {
        val user = repository.getById(userId)
            ?: return OpResult.Failure("User not found")

        if (!user.active) {
            return OpResult.Failure("User is already inactive")
        }

        val deactivated = user.copy(active = false)
        repository.update(deactivated)
        return OpResult.Success(deactivated)
    }
}
