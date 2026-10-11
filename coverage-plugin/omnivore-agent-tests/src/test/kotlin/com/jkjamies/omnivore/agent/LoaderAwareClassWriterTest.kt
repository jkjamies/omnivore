package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.LoaderAwareClassWriter
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.assertThrows

class LoaderAwareClassWriterTest {

    // getCommonSuperClass is protected (ASM calls it during frame computation).
    private val writer = LoaderAwareClassWriter(javaClass.classLoader)
    private val method = LoaderAwareClassWriter::class.java
        .getDeclaredMethod("getCommonSuperClass", String::class.java, String::class.java)
        .apply { isAccessible = true }

    private fun common(a: String, b: String): String = try {
        method.invoke(writer, a, b) as String
    } catch (e: java.lang.reflect.InvocationTargetException) {
        throw e.targetException
    }

    @Test
    fun `resolves the nearest common superclass`() {
        assertEquals("java/lang/Number", common("java/lang/Integer", "java/lang/Long"))
        assertEquals("java/lang/Number", common("java/lang/Number", "java/lang/Integer"))
    }

    @Test
    fun `an unresolvable type fails the transform instead of guessing Object`() {
        // Guessing Object can produce a frame the verifier rejects; throwing
        // makes the transformer skip the class and leave it uninstrumented.
        assertThrows<TypeNotPresentException> {
            common("com/example/DoesNotExist", "java/lang/Integer")
        }
    }
}
