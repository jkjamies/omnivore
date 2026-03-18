package com.example.app

/**
 * String utility functions — some will be tested, some won't.
 */
object StringUtils {

    fun reverse(input: String): String {
        return input.reversed()
    }

    fun isPalindrome(input: String): Boolean {
        val cleaned = input.lowercase().filter { it.isLetterOrDigit() }
        return cleaned == cleaned.reversed()
    }

    fun truncate(input: String, maxLength: Int): String {
        if (maxLength < 0) throw IllegalArgumentException("maxLength must be >= 0")
        if (input.length <= maxLength) return input
        if (maxLength <= 3) return input.take(maxLength)
        return input.take(maxLength - 3) + "..."
    }

    fun countWords(input: String): Int {
        if (input.isBlank()) return 0
        return input.trim().split("\\s+".toRegex()).size
    }

    /** This function is intentionally never tested — should show as uncovered */
    fun toCamelCase(input: String): String {
        return input.split("_", "-", " ")
            .mapIndexed { index, word ->
                if (index == 0) word.lowercase()
                else word.replaceFirstChar { it.uppercase() }
            }
            .joinToString("")
    }
}
