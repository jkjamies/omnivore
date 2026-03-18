package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.ComposeDetector
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ComposeDetectorTest {

    @Test
    fun `detects ComposableSingletons classes`() {
        assertTrue(ComposeDetector.isGeneratedClass("com/example/ComposableSingletons\$MainActivityKt"))
        assertTrue(ComposeDetector.isGeneratedClass("ComposableSingletons\$AppKt"))
    }

    @Test
    fun `detects LiveLiterals classes`() {
        assertTrue(ComposeDetector.isGeneratedClass("com/example/LiveLiterals\$MainActivityKt"))
    }

    @Test
    fun `does not flag regular classes`() {
        assertFalse(ComposeDetector.isGeneratedClass("com/example/MainActivity"))
        assertFalse(ComposeDetector.isGeneratedClass("com/example/UserRepository"))
        assertFalse(ComposeDetector.isGeneratedClass("com/example/ui/HomeScreen"))
    }

    @Test
    fun `detects Compose lambda groups`() {
        assertTrue(ComposeDetector.isComposeLambdaGroup("\$lambda-0"))
        assertTrue(ComposeDetector.isComposeLambdaGroup("invoke\$lambda-2"))
        assertTrue(ComposeDetector.isComposeLambdaGroup("content\$lambda\$0"))
    }

    @Test
    fun `does not flag regular lambdas`() {
        assertFalse(ComposeDetector.isComposeLambdaGroup("invoke"))
        assertFalse(ComposeDetector.isComposeLambdaGroup("onClick"))
    }

    @Test
    fun `matches exclude patterns`() {
        assertTrue(
            ComposeDetector.matchesExcludePattern(
                "com/example/ComposableSingletons\$Test",
                listOf("*ComposableSingletons*")
            )
        )
        assertFalse(
            ComposeDetector.matchesExcludePattern(
                "com/example/UserRepo",
                listOf("*ComposableSingletons*")
            )
        )
    }
}
