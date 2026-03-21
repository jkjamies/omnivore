package com.example.app.presentation

import com.example.core.model.OpResult
import com.example.core.usecase.AddUserUseCase
import com.example.core.usecase.DeactivateUserUseCase
import com.example.core.usecase.GetUsersUseCase
import com.example.core.usecase.RemoveUserUseCase

/**
 * MVI store for the user list — processes intents and produces state + effects.
 * Pure Kotlin (no Android dependencies), suitable for KMP.
 */
class UserListStore(
    private val getUsersUseCase: GetUsersUseCase,
    private val addUserUseCase: AddUserUseCase,
    private val deactivateUserUseCase: DeactivateUserUseCase,
    private val removeUserUseCase: RemoveUserUseCase,
) {
    private var _state = UserListContract.State()
    val state: UserListContract.State get() = _state

    private val _effects = mutableListOf<UserListContract.Effect>()
    fun consumeEffects(): List<UserListContract.Effect> {
        val effects = _effects.toList()
        _effects.clear()
        return effects
    }

    fun processIntent(intent: UserListContract.Intent) {
        when (intent) {
            is UserListContract.Intent.LoadUsers -> loadUsers()
            is UserListContract.Intent.AddUser -> addUser(intent.id, intent.name, intent.email)
            is UserListContract.Intent.DeactivateUser -> deactivateUser(intent.userId)
            is UserListContract.Intent.RemoveUser -> removeUser(intent.userId)
            is UserListContract.Intent.SetFilter -> setFilter(intent.filter)
            is UserListContract.Intent.Search -> search(intent.query)
        }
    }

    private fun loadUsers() {
        _state = _state.copy(isLoading = true, error = null)
        try {
            val users = getUsersUseCase(_state.filter)
            _state = _state.copy(users = users, isLoading = false)
        } catch (e: Exception) {
            _state = _state.copy(isLoading = false, error = e.message)
            _effects.add(UserListContract.Effect.ShowError(e.message ?: "Unknown error"))
        }
    }

    private fun addUser(id: Int, name: String, email: String) {
        when (val result = addUserUseCase(id, name, email)) {
            is OpResult.Success -> {
                _effects.add(UserListContract.Effect.UserAdded(result.value))
                loadUsers()
            }
            is OpResult.Failure -> {
                _effects.add(UserListContract.Effect.ShowError(result.message))
            }
        }
    }

    private fun deactivateUser(userId: Int) {
        when (val result = deactivateUserUseCase(userId)) {
            is OpResult.Success -> loadUsers()
            is OpResult.Failure -> _effects.add(UserListContract.Effect.ShowError(result.message))
        }
    }

    private fun removeUser(userId: Int) {
        when (val result = removeUserUseCase(userId)) {
            is OpResult.Success -> {
                _effects.add(UserListContract.Effect.UserRemoved)
                loadUsers()
            }
            is OpResult.Failure -> _effects.add(UserListContract.Effect.ShowError(result.message))
        }
    }

    private fun setFilter(filter: com.example.core.usecase.GetUsersUseCase.Filter) {
        _state = _state.copy(filter = filter)
        loadUsers()
    }

    private fun search(query: String) {
        _state = _state.copy(searchQuery = query)
    }
}
