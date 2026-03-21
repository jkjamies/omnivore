package com.example.app.util

/**
 * Simple calculator with branching logic — good for testing line + branch coverage.
 */
class Calculator {

    fun add(a: Int, b: Int): Int {
        return a + b
    }

    fun subtract(a: Int, b: Int): Int {
        return a - b
    }

    fun multiply(a: Int, b: Int): Int {
        return a * b
    }

    fun divide(a: Int, b: Int): Int {
        if (b == 0) {
            throw IllegalArgumentException("Cannot divide by zero")
        }
        return a / b
    }

    fun classify(value: Int): String {
        return when {
            value < 0 -> "negative"
            value == 0 -> "zero"
            value < 10 -> "small"
            value < 100 -> "medium"
            else -> "large"
        }
    }

    fun fibonacci(n: Int): Long {
        if (n <= 0) return 0
        if (n == 1) return 1

        var a = 0L
        var b = 1L
        for (i in 2..n) {
            val temp = a + b
            a = b
            b = temp
        }
        return b
    }
}
