package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.KotlinDetector
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class KotlinDetectorTest {

    @Test
    fun `detects data class generated methods`() {
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("copy"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("copy\$default"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("toString"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("hashCode"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("equals"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("component1"))
        assertTrue(KotlinDetector.isDataClassGeneratedMethod("component2"))
    }

    @Test
    fun `regular methods are not data class methods`() {
        assertFalse(KotlinDetector.isDataClassGeneratedMethod("getName"))
        assertFalse(KotlinDetector.isDataClassGeneratedMethod("process"))
        assertFalse(KotlinDetector.isDataClassGeneratedMethod("toDto"))
    }
}
