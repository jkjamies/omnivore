package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.ComposeDetector
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

class ComposeDetectorTest {

    @Test
    fun `detects ComposableSingletons as generated class`() {
        assertTrue(ComposeDetector.isGeneratedClass("com/example/ComposableSingletons\$MyScreenKt"))
        assertTrue(ComposeDetector.isGeneratedClass("ComposableSingletons\$AppKt"))
    }

    @Test
    fun `detects LiveLiterals as generated class`() {
        assertTrue(ComposeDetector.isGeneratedClass("com/example/LiveLiterals\$MyScreenKt"))
    }

    @Test
    fun `regular classes are not detected as generated`() {
        assertFalse(ComposeDetector.isGeneratedClass("com/example/MyViewModel"))
        assertFalse(ComposeDetector.isGeneratedClass("com/example/UserRepository"))
        assertFalse(ComposeDetector.isGeneratedClass("com/example/ui/LoginScreen"))
    }

    @Test
    fun `detects Compose lambda groups`() {
        assertTrue(ComposeDetector.isComposeLambdaGroup("\$lambda-0"))
        assertTrue(ComposeDetector.isComposeLambdaGroup("invoke\$lambda-1"))
        assertTrue(ComposeDetector.isComposeLambdaGroup("content\$lambda\$0"))
    }

    @Test
    fun `regular methods are not lambda groups`() {
        assertFalse(ComposeDetector.isComposeLambdaGroup("onClick"))
        assertFalse(ComposeDetector.isComposeLambdaGroup("toString"))
        assertFalse(ComposeDetector.isComposeLambdaGroup("lambda")) // no $ prefix
    }

    @Test
    fun `matches exclude patterns with glob`() {
        val patterns = listOf("*ComposableSingletons*", "*\$lambda-*")

        assertTrue(
            ComposeDetector.matchesExcludePattern(
                "com/example/ComposableSingletons\$MainKt",
                patterns
            )
        )
        assertFalse(
            ComposeDetector.matchesExcludePattern(
                "com/example/MyScreen",
                patterns
            )
        )
    }

    @Test
    fun `detects Compose boilerplate methods`() {
        assertTrue(ComposeDetector.isComposeBoilerplateMethod("startRestartGroup"))
        assertTrue(ComposeDetector.isComposeBoilerplateMethod("endRestartGroup"))
        assertTrue(ComposeDetector.isComposeBoilerplateMethod("skipToGroupEnd"))
        assertFalse(ComposeDetector.isComposeBoilerplateMethod("onClick"))
    }
}
