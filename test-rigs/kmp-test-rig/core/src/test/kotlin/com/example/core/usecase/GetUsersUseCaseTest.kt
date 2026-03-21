package com.example.core.usecase

import com.example.core.model.User
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class GetUsersUseCaseTest : FunSpec({

    lateinit var repository: FakeUserRepository
    lateinit var useCase: GetUsersUseCase

    beforeEach {
        repository = FakeUserRepository()
        useCase = GetUsersUseCase(repository)
    }

    test("returns all users with ALL filter") {
        repository.add(User(1, "Alice", "alice@example.com"))
        repository.add(User(2, "Bob", "bob@example.com", active = false))

        val result = useCase(GetUsersUseCase.Filter.ALL)
        result.size shouldBe 2
    }

    test("returns only active users with ACTIVE filter") {
        repository.add(User(1, "Alice", "alice@example.com"))
        repository.add(User(2, "Bob", "bob@example.com", active = false))

        val result = useCase(GetUsersUseCase.Filter.ACTIVE)
        result.size shouldBe 1
        result[0].name shouldBe "Alice"
    }

    // Intentionally not testing: INACTIVE filter
})
