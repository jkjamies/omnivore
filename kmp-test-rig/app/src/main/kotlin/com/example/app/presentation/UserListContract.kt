package com.example.app.presentation

import com.example.core.model.User
import com.example.core.usecase.GetUsersUseCase

/**
 * MVI contract for the user list screen.
 */
object UserListContract {

    data class State(
        val users: List<User> = emptyList(),
        val filter: GetUsersUseCase.Filter = GetUsersUseCase.Filter.ALL,
        val isLoading: Boolean = false,
        val error: String? = null,
        val searchQuery: String = "",
    ) {
        val filteredUsers: List<User>
            get() = if (searchQuery.isBlank()) users
            else users.filter { it.matchesSearch(searchQuery) }

        val activeCount: Int get() = users.count { it.active }
        val inactiveCount: Int get() = users.count { !it.active }
    }

    sealed class Intent {
        data object LoadUsers : Intent()
        data class AddUser(val id: Int, val name: String, val email: String) : Intent()
        data class DeactivateUser(val userId: Int) : Intent()
        data class RemoveUser(val userId: Int) : Intent()
        data class SetFilter(val filter: GetUsersUseCase.Filter) : Intent()
        data class Search(val query: String) : Intent()
    }

    sealed class Effect {
        data class ShowError(val message: String) : Effect()
        data class UserAdded(val user: User) : Effect()
        data object UserRemoved : Effect()
    }
}
