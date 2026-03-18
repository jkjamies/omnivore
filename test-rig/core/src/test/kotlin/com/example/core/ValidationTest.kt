package com.example.core

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ValidationTest {

    @Test
    fun `valid email passes`() {
        assertTrue(Validation.isValidEmail("user@example.com"))
    }

    @Test
    fun `blank email fails`() {
        assertFalse(Validation.isValidEmail(""))
        assertFalse(Validation.isValidEmail("   "))
    }

    @Test
    fun `email without at fails`() {
        assertFalse(Validation.isValidEmail("userexample.com"))
    }

    // Intentionally not testing: no dot after @, dot at end, isValidName edge cases

    @Test
    fun `sanitize trims and collapses whitespace`() {
        assertEquals("hello world", Validation.sanitize("  hello   world  "))
    }

    @Test
    fun `valid id is positive`() {
        assertTrue(Validation.isValidId(1))
        assertFalse(Validation.isValidId(0))
        assertFalse(Validation.isValidId(-5))
    }
}
