package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.GlobPattern
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/**
 * Regression tests for include/exclude pattern matching.
 *
 * The previous implementation escaped only `.` when translating a glob to a
 * regex, so any pattern containing another regex metacharacter was silently
 * reinterpreted. `$` is the one that matters in practice — it appears in the
 * name of every JVM inner and synthetic class — and an exclusion that mentioned
 * it matched nothing at all, so the class kept being instrumented with no
 * warning.
 */
class GlobPatternTest {

    @Test
    fun `matches plain class names`() {
        assertTrue(GlobPattern.matches("com.example.Foo", "com.example.Foo"))
        assertFalse(GlobPattern.matches("com.example.Foo", "com.example.Bar"))
    }

    @Test
    fun `dot is literal, not any-character`() {
        assertFalse(GlobPattern.matches("com.example.Foo", "comXexampleXFoo"))
    }

    @Test
    fun `star matches any run of characters`() {
        assertTrue(GlobPattern.matches("com.example.*", "com.example.Foo"))
        assertTrue(GlobPattern.matches("com.example.*", "com.example.sub.Foo"))
        assertFalse(GlobPattern.matches("com.example.*", "com.other.Foo"))
    }

    @Test
    fun `question mark matches exactly one character`() {
        assertTrue(GlobPattern.matches("com.example.Foo?", "com.example.Foo1"))
        assertFalse(GlobPattern.matches("com.example.Foo?", "com.example.Foo12"))
    }

    @Test
    fun `dollar sign in inner class names is literal`() {
        // The original bug: '$' compiled to an end-of-input anchor, so this
        // exclusion never matched and the inner class stayed instrumented.
        assertTrue(GlobPattern.matches("com.example.Foo\$Bar", "com.example.Foo\$Bar"))
        assertTrue(GlobPattern.matches("com.example.Foo\$*", "com.example.Foo\$Inner"))
        assertFalse(GlobPattern.matches("com.example.Foo\$Bar", "com.example.FooBar"))
    }

    @Test
    fun `kotlin synthetic suffixes are matchable`() {
        assertTrue(
            GlobPattern.matches("*\$\$serializer", "com.example.User\$\$serializer")
        )
        assertTrue(
            GlobPattern.matches("com.example.*\$WhenMappings", "com.example.Foo\$WhenMappings")
        )
    }

    @Test
    fun `other regex metacharacters are literal`() {
        assertTrue(GlobPattern.matches("com.example.Foo+Bar", "com.example.Foo+Bar"))
        assertFalse(GlobPattern.matches("com.example.Foo+Bar", "com.example.FooooBar"))
        assertTrue(GlobPattern.matches("com.example.A(1)", "com.example.A(1)"))
        assertTrue(GlobPattern.matches("com.example.A[x]", "com.example.A[x]"))
        assertTrue(GlobPattern.matches("com.example.A{1}", "com.example.A{1}"))
        assertTrue(GlobPattern.matches("com.example.A|B", "com.example.A|B"))
        assertTrue(GlobPattern.matches("com.example.^Odd", "com.example.^Odd"))
    }

    @Test
    fun `unbalanced brackets do not throw`() {
        // These used to blow up PatternSyntaxException mid-instrumentation.
        assertTrue(GlobPattern.matches("com.example.A(", "com.example.A("))
        assertTrue(GlobPattern.matches("com.example.A[", "com.example.A["))
    }

    @Test
    fun `regex prefix opts into full regex syntax`() {
        assertTrue(GlobPattern.matches("regex:com\\.example\\.(Foo|Bar)", "com.example.Bar"))
        assertFalse(GlobPattern.matches("regex:com\\.example\\.(Foo|Bar)", "com.example.Baz"))
    }

    @Test
    fun `invalid regex never matches instead of throwing`() {
        assertFalse(GlobPattern.matches("regex:com.example.(", "com.example.anything"))
    }

    @Test
    fun `wildcards still apply around escaped metacharacters`() {
        assertTrue(GlobPattern.matches("*.Foo\$*", "com.example.Foo\$Inner"))
        assertFalse(GlobPattern.matches("*.Foo\$*", "com.example.FooInner"))
    }
}

/**
 * Regression tests for include/exclude precedence.
 *
 * The built-in infrastructure skip list contains broad prefixes such as
 * `com.google.` and `com.squareup.`. It ran before user includes, so anyone
 * whose own code lives under one of those got no coverage, no warning, and no
 * way to override it.
 */
class IncludeOverridesInfrastructureTest {

    private fun transformerWith(includes: List<String>) =
        com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer(
            com.jkjamies.omnivore.agent.runtime.ExecutionDataStore(),
            com.jkjamies.omnivore.agent.runtime.ProbeMap(),
            AgentConfig(composeFilterEnabled = false, includes = includes),
        )

    private fun classBytes(internalName: String): ByteArray {
        val cw = org.objectweb.asm.ClassWriter(org.objectweb.asm.ClassWriter.COMPUTE_FRAMES)
        cw.visit(
            org.objectweb.asm.Opcodes.V17,
            org.objectweb.asm.Opcodes.ACC_PUBLIC,
            internalName, null, "java/lang/Object", null,
        )
        cw.visitSource(internalName.substringAfterLast('/') + ".java", null)
        val mv = cw.visitMethod(
            org.objectweb.asm.Opcodes.ACC_PUBLIC or org.objectweb.asm.Opcodes.ACC_STATIC,
            "go", "()I", null, null,
        )
        mv.visitCode()
        val l = org.objectweb.asm.Label()
        mv.visitLabel(l)
        mv.visitLineNumber(7, l)
        mv.visitInsn(org.objectweb.asm.Opcodes.ICONST_1)
        mv.visitInsn(org.objectweb.asm.Opcodes.IRETURN)
        mv.visitMaxs(1, 0)
        mv.visitEnd()
        cw.visitEnd()
        return cw.toByteArray()
    }

    @Test
    fun `infrastructure prefixes are skipped by default`() {
        val transformer = transformerWith(emptyList())
        val result = transformer.transform(
            javaClass.classLoader, "com/google/thing/Widget", null, null,
            classBytes("com/google/thing/Widget"),
        )
        assertNull(result, "com.google.* should be skipped when not explicitly included")
    }

    @Test
    fun `an explicit include overrides the infrastructure skip list`() {
        val transformer = transformerWith(listOf("com.google.thing.*"))
        val result = transformer.transform(
            javaClass.classLoader, "com/google/thing/Widget", null, null,
            classBytes("com/google/thing/Widget"),
        )
        assertNotNull(
            result,
            "a class the user explicitly asked for must be instrumented even if " +
                "its package matches a built-in skip prefix",
        )
    }

    @Test
    fun `an unrelated include does not resurrect infrastructure classes`() {
        val transformer = transformerWith(listOf("com.example.*"))
        val result = transformer.transform(
            javaClass.classLoader, "com/google/thing/Widget", null, null,
            classBytes("com/google/thing/Widget"),
        )
        assertNull(result)
    }
}
