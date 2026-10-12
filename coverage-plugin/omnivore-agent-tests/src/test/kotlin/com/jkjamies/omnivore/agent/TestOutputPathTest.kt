package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer.Companion.isTestOutputPath
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Test classes are recognised by where the build wrote them. Each layout a
 * supported build tool produces needs a case here: when AGP 9 moved Kotlin
 * output under intermediates/, unit-test classes went unrecognised and were
 * reported as fully covered production code.
 */
class TestOutputPathTest {

    @Test
    fun `gradle test source sets`() {
        assertTrue(isTestOutputPath("/p/core/build/classes/kotlin/test/"))
        assertTrue(isTestOutputPath("/p/core/build/classes/java/test/"))
    }

    @Test
    fun `agp 8 kotlin output`() {
        assertTrue(isTestOutputPath("/p/app/build/tmp/kotlin-classes/debugUnitTest/"))
        assertTrue(isTestOutputPath("/p/app/build/tmp/kotlin-classes/debugAndroidTest/"))
    }

    @Test
    fun `agp 9 built-in kotlin output`() {
        assertTrue(
            isTestOutputPath(
                "/p/app/build/intermediates/built_in_kotlinc/debugUnitTest/compileDebugUnitTestKotlin/classes/",
            ),
        )
    }

    @Test
    fun `agp javac output`() {
        assertTrue(
            isTestOutputPath(
                "/p/app/build/intermediates/javac/debugUnitTest/compileDebugUnitTestJavaWithJavac/classes/",
            ),
        )
    }

    @Test
    fun `production output is not test output`() {
        assertFalse(isTestOutputPath("/p/core/build/classes/kotlin/main/"))
        assertFalse(isTestOutputPath("/p/app/build/tmp/kotlin-classes/debug/"))
        assertFalse(
            isTestOutputPath("/p/app/build/intermediates/built_in_kotlinc/debug/compileDebugKotlin/classes/"),
        )
        assertFalse(
            isTestOutputPath("/p/app/build/intermediates/javac/debug/compileDebugJavaWithJavac/classes/"),
        )
        // A production class that merely has "test" in its package path.
        assertFalse(isTestOutputPath("/p/app/build/intermediates/built_in_kotlinc/debug/compileDebugKotlin/classes/com/x/test/"))
    }
}
