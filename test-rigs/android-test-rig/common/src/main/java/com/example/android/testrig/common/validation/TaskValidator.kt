package com.example.android.testrig.common.validation

import com.example.android.testrig.domain.model.Task

/**
 * Validation utilities for task data.
 */
object TaskValidator {

    data class ValidationResult(
        val isValid: Boolean,
        val errors: List<String> = emptyList(),
    )

    fun validateTitle(title: String): ValidationResult {
        val errors = mutableListOf<String>()
        if (title.isBlank()) {
            errors.add("Title cannot be blank")
        }
        if (title.length > 200) {
            errors.add("Title cannot exceed 200 characters")
        }
        if (title.trim().length < 3) {
            errors.add("Title must be at least 3 characters")
        }
        return ValidationResult(errors.isEmpty(), errors)
    }

    fun validateDescription(description: String): ValidationResult {
        val errors = mutableListOf<String>()
        if (description.length > 2000) {
            errors.add("Description cannot exceed 2000 characters")
        }
        return ValidationResult(errors.isEmpty(), errors)
    }

    fun validateTask(task: Task): ValidationResult {
        val titleResult = validateTitle(task.title)
        val descResult = validateDescription(task.description)
        val allErrors = titleResult.errors + descResult.errors
        return ValidationResult(allErrors.isEmpty(), allErrors)
    }

    fun sanitizeTitle(title: String): String {
        return title.trim().replace(Regex("\\s+"), " ")
    }

    fun sanitizeDescription(description: String): String {
        val cleaned = description.trim().replace(Regex("\\s+"), " ")
        return if (cleaned.length > 2000) cleaned.take(2000) else cleaned
    }
}
