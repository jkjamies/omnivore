package com.example.core

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

class ResultTest {

    @Test
    fun `success holds value`() {
        val result = OpResult.Success(42)
        assertTrue(result.isSuccess())
        assertFalse(result.isFailure())
        assertEquals(42, result.getOrNull())
        assertNull(result.errorOrNull())
    }

    @Test
    fun `failure holds message`() {
        val result = OpResult.Failure("bad input")
        assertFalse(result.isSuccess())
        assertTrue(result.isFailure())
        assertNull(result.getOrNull())
        assertEquals("bad input", result.errorOrNull())
    }

    // Intentionally not testing: map()
}
