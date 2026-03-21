package com.example.app.presentation

import com.example.app.data.repository.InMemoryUserRepository
import com.example.core.model.User
import com.example.core.usecase.AddUserUseCase
import com.example.core.usecase.DeactivateUserUseCase
import com.example.core.usecase.GetUsersUseCase
import com.example.core.usecase.RemoveUserUseCase
import io.kotest.core.spec.style.FunSpec
import io.kotest.matchers.collections.shouldHaveSize
import io.kotest.matchers.shouldBe
import io.kotest.matchers.types.shouldBeInstanceOf

class UserListStoreTest : FunSpec({

    lateinit var repository: InMemoryUserRepository
    lateinit var store: UserListStore

    beforeEach {
        repository = InMemoryUserRepository()
        store = UserListStore(
            getUsersUseCase = GetUsersUseCase(repository),
            addUserUseCase = AddUserUseCase(repository),
            deactivateUserUseCase = DeactivateUserUseCase(repository),
            removeUserUseCase = RemoveUserUseCase(repository),
        )
    }

    test("load users populates state") {
        repository.add(User(1, "Alice", "alice@example.com"))
        repository.add(User(2, "Bob", "bob@example.com"))

        store.processIntent(UserListContract.Intent.LoadUsers)

        store.state.users shouldHaveSize 2
        store.state.isLoading shouldBe false
    }

    test("add user via intent") {
        store.processIntent(UserListContract.Intent.AddUser(1, "Alice", "alice@example.com"))

        val effects = store.consumeEffects()
        effects.any { it is UserListContract.Effect.UserAdded } shouldBe true
        store.state.users shouldHaveSize 1
    }

    test("search filters displayed users") {
        repository.add(User(1, "Alice", "alice@example.com"))
        repository.add(User(2, "Bob", "bob@example.com"))

        store.processIntent(UserListContract.Intent.LoadUsers)
        store.processIntent(UserListContract.Intent.Search("alice"))

        store.state.filteredUsers shouldHaveSize 1
    }

    // Intentionally not testing: DeactivateUser, RemoveUser, SetFilter, error effects
})
