package com.example.core

/**
 * A simple result type for operations that can fail with a message.
 */
sealed class OpResult<out T> {
    data class Success<T>(val value: T) : OpResult<T>()
    data class Failure(val message: String) : OpResult<Nothing>()

    fun isSuccess(): Boolean = this is Success
    fun isFailure(): Boolean = this is Failure

    fun getOrNull(): T? = when (this) {
        is Success -> value
        is Failure -> null
    }

    fun errorOrNull(): String? = when (this) {
        is Success -> null
        is Failure -> message
    }

    fun <R> map(transform: (T) -> R): OpResult<R> = when (this) {
        is Success -> Success(transform(value))
        is Failure -> this
    }
}
