package com.example.core.usecase

import com.example.core.model.User
import com.example.core.repository.UserRepository
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.shouldBe

class AddUserUseCaseTest : FunSpec({

    lateinit var repository: FakeUserRepository
    lateinit var useCase: AddUserUseCase

    beforeEach {
        repository = FakeUserRepository()
        useCase = AddUserUseCase(repository)
    }

    test("adds user with valid data") {
        val result = useCase(1, "Alice", "alice@example.com")
        result.isSuccess() shouldBe true
        result.getOrNull()!!.name shouldBe "Alice"
    }

    test("rejects duplicate id") {
        useCase(1, "Alice", "alice@example.com")
        val result = useCase(1, "Bob", "bob@example.com")
        result.isFailure() shouldBe true
    }

    test("rejects invalid email") {
        val result = useCase(1, "Alice", "not-an-email")
        result.isFailure() shouldBe true
        result.errorOrNull() shouldBe "Invalid email"
    }

    // Intentionally not testing: invalid name, invalid id
})

/** Simple in-memory implementation for testing use cases. */
internal class FakeUserRepository : UserRepository {
    private val users = mutableMapOf<Int, User>()
    override fun getAll() = users.values.toList()
    override fun getById(id: Int) = users[id]
    override fun add(user: User) { users[user.id] = user }
    override fun update(user: User) { users[user.id] = user }
    override fun remove(id: Int) = users.remove(id) != null
    override fun findByEmail(email: String) = users.values.find { it.email.equals(email, ignoreCase = true) }
}
