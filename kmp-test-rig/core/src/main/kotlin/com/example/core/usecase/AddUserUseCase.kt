package com.example.core.usecase

import com.example.core.Validation
import com.example.core.model.OpResult
import com.example.core.model.User
import com.example.core.repository.UserRepository

class AddUserUseCase(private val repository: UserRepository) {

    operator fun invoke(id: Int, name: String, email: String): OpResult<User> {
        if (!Validation.isValidId(id)) {
            return OpResult.Failure("Invalid user ID")
        }
        if (!Validation.isValidName(name)) {
            return OpResult.Failure("Invalid name")
        }
        if (!Validation.isValidEmail(email)) {
            return OpResult.Failure("Invalid email")
        }
        if (repository.getById(id) != null) {
            return OpResult.Failure("User already exists")
        }

        val user = User(
            id = id,
            name = Validation.sanitize(name),
            email = email,
        )
        repository.add(user)
        return OpResult.Success(user)
    }
}
