package com.example.core.usecase

import com.example.core.model.OpResult
import com.example.core.repository.UserRepository

class RemoveUserUseCase(private val repository: UserRepository) {

    operator fun invoke(userId: Int): OpResult<Unit> {
        val removed = repository.remove(userId)
        return if (removed) {
            OpResult.Success(Unit)
        } else {
            OpResult.Failure("User not found")
        }
    }
}
