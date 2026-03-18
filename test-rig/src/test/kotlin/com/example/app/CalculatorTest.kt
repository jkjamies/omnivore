package com.example.app

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class CalculatorTest {

    private val calc = Calculator()

    @Test
    fun `add works`() {
        assertEquals(5, calc.add(2, 3))
    }

    @Test
    fun `subtract works`() {
        assertEquals(1, calc.subtract(3, 2))
    }

    // multiply is intentionally not tested

    @Test
    fun `divide works`() {
        assertEquals(5, calc.divide(10, 2))
    }

    @Test
    fun `divide by zero throws`() {
        assertThrows(IllegalArgumentException::class.java) {
            calc.divide(1, 0)
        }
    }

    @Test
    fun `classify negative`() {
        assertEquals("negative", calc.classify(-5))
    }

    @Test
    fun `classify zero`() {
        assertEquals("zero", calc.classify(0))
    }

    @Test
    fun `classify small`() {
        assertEquals("small", calc.classify(5))
    }

    // medium and large branches intentionally not tested

    @Test
    fun `fibonacci base cases`() {
        assertEquals(0, calc.fibonacci(0))
        assertEquals(1, calc.fibonacci(1))
    }

    @Test
    fun `fibonacci computes correctly`() {
        assertEquals(8, calc.fibonacci(6))
    }
}
