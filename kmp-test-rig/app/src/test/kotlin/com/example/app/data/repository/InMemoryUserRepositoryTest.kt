package com.example.app.data.repository

import com.example.core.model.User
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.nulls.shouldBeNull
import io.kotest.matchers.shouldBe

class InMemoryUserRepositoryTest : FunSpec({

    lateinit var repository: InMemoryUserRepository

    beforeEach {
        repository = InMemoryUserRepository()
    }

    test("add and get by id") {
        val user = User(1, "Alice", "alice@example.com")
        repository.add(user)
        repository.getById(1) shouldBe user
    }

    test("get nonexistent returns null") {
        repository.getById(999).shouldBeNull()
    }

    test("remove returns true for existing") {
        repository.add(User(1, "Alice", "alice@example.com"))
        repository.remove(1) shouldBe true
        repository.getById(1).shouldBeNull()
    }

    test("remove returns false for nonexistent") {
        repository.remove(999) shouldBe false
    }

    // Intentionally not testing: update, findByEmail, getAll
})
