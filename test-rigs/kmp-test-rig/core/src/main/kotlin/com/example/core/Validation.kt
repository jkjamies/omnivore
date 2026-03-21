package com.example.core

/**
 * Shared validation utilities used across modules.
 */
object Validation {

    fun isValidEmail(email: String): Boolean {
        if (email.isBlank()) return false
        val atIndex = email.indexOf('@')
        if (atIndex <= 0) return false
        val dotIndex = email.lastIndexOf('.')
        if (dotIndex <= atIndex + 1) return false
        if (dotIndex >= email.length - 1) return false
        return true
    }

    fun isValidName(name: String): Boolean {
        if (name.isBlank()) return false
        if (name.length < 2) return false
        if (name.length > 100) return false
        return name.all { it.isLetter() || it.isWhitespace() || it == '-' || it == '\'' }
    }

    fun sanitize(input: String): String {
        return input.trim().replace(Regex("\\s+"), " ")
    }

    fun isValidId(id: Int): Boolean {
        return id > 0
    }
}
