package com.jkjamies.omnivore.agent.instrumentation

import org.objectweb.asm.tree.ClassNode
import org.objectweb.asm.tree.MethodNode

/**
 * Detects Kotlin-specific bytecode patterns that need special handling
 * during instrumentation.
 *
 * Kotlin generates various synthetic constructs that can confuse coverage tools:
 * - Inline function bodies (duplicated at call sites)
 * - Data class generated methods (equals, hashCode, toString, copy, componentN)
 * - Default parameter bridge methods
 * - Coroutine state machines
 * - Companion object accessors
 */
object KotlinDetector {

    private const val KOTLIN_METADATA_DESCRIPTOR = "Lkotlin/Metadata;"

    /**
     * Check if a class is a Kotlin class (has @Metadata annotation).
     */
    fun isKotlinClass(classNode: ClassNode): Boolean {
        return classNode.visibleAnnotations?.any {
            it.desc == KOTLIN_METADATA_DESCRIPTOR
        } == true
    }

    /**
     * Check if a method is a Kotlin compiler-generated synthetic method
     * that should not be counted toward coverage.
     */
    fun isSyntheticBridgeMethod(methodNode: MethodNode): Boolean {
        val name = methodNode.name ?: return false

        // Default parameter bridge methods
        if (name.endsWith("\$default")) return true

        // When-mappings table
        if (name.startsWith("\$WhenMappings")) return true

        return false
    }

    /**
     * Check if a method is a data class generated method.
     * These are auto-generated and testing them adds no value.
     */
    fun isDataClassGeneratedMethod(methodName: String): Boolean {
        return methodName in setOf(
            "copy",
            "copy\$default",
            "toString",
            "hashCode",
            "equals",
        ) || methodName.startsWith("component")
    }

    /**
     * Check if a method is a coroutine state machine continuation.
     * These are implementation details of suspend functions.
     */
    fun isCoroutineContinuation(className: String): Boolean {
        return className.contains("\$\$inlined\$") ||
            className.endsWith("\$1") && className.contains("Continuation")
    }
}
