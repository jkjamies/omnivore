package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.Label
import org.objectweb.asm.Opcodes

/**
 * End-to-end tests that:
 * 1. Generate test classes with ASM
 * 2. Instrument them with OmnivoreClassTransformer
 * 3. Load them into the JVM via a custom classloader
 * 4. Execute methods on them
 * 5. Verify probe data in the ExecutionDataStore
 */
class EndToEndInstrumentationTest {

    private lateinit var dataStore: ExecutionDataStore
    private lateinit var transformer: OmnivoreClassTransformer

    @BeforeEach
    fun setUp() {
        dataStore = ExecutionDataStore()
        OmnivoreRuntime.dataStoreOverride = dataStore
        transformer = OmnivoreClassTransformer(dataStore, config = AgentConfig(composeFilterEnabled = false))
    }

    @AfterEach
    fun tearDown() {
        OmnivoreRuntime.dataStoreOverride = null
    }

    /**
     * Custom classloader that can define a class from raw bytes.
     * Uses the parent classloader for everything except the specific test class.
     */
    private class InstrumentedClassLoader(
        private val className: String,
        private val bytecode: ByteArray,
        parent: ClassLoader,
    ) : ClassLoader(parent) {
        override fun loadClass(name: String, resolve: Boolean): Class<*> {
            if (name == className) {
                return defineClass(name, bytecode, 0, bytecode.size).also {
                    if (resolve) resolveClass(it)
                }
            }
            return super.loadClass(name, resolve)
        }
    }

    /** Instrument bytecode and load the resulting class. */
    private fun instrumentAndLoad(internalName: String, bytecode: ByteArray): Class<*> {
        val instrumented = transformer.transform(null, internalName, null, null, bytecode)
        assertNotNull(instrumented, "$internalName should be instrumented")

        val dotName = internalName.replace('/', '.')
        val loader = InstrumentedClassLoader(dotName, instrumented!!, this::class.java.classLoader)
        return loader.loadClass(dotName)
    }

    // ---- Test class generators ----

    /**
     * public class test/e2e/SimpleCalc {
     *     public static int add(int a, int b) {
     *         int result = a + b; // line 10
     *         return result;      // line 11
     *     }
     * }
     */
    private fun generateSimpleCalc(): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, "test/e2e/SimpleCalc", null, "java/lang/Object", null)

        val init = cw.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null)
        init.visitCode()
        init.visitVarInsn(Opcodes.ALOAD, 0)
        init.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false)
        init.visitInsn(Opcodes.RETURN)
        init.visitMaxs(1, 1)
        init.visitEnd()

        val add = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "add", "(II)I", null, null)
        add.visitCode()
        val l0 = Label()
        add.visitLabel(l0)
        add.visitLineNumber(10, l0)
        add.visitVarInsn(Opcodes.ILOAD, 0)
        add.visitVarInsn(Opcodes.ILOAD, 1)
        add.visitInsn(Opcodes.IADD)
        add.visitVarInsn(Opcodes.ISTORE, 2)
        val l1 = Label()
        add.visitLabel(l1)
        add.visitLineNumber(11, l1)
        add.visitVarInsn(Opcodes.ILOAD, 2)
        add.visitInsn(Opcodes.IRETURN)
        add.visitMaxs(2, 3)
        add.visitEnd()

        cw.visitEnd()
        return cw.toByteArray()
    }

    /**
     * public class test/e2e/BranchLogic {
     *     public static String classify(int n) {
     *         if (n > 0) {           // line 10 + branch probe
     *             return "positive"; // line 11
     *         } else {
     *             return "non-positive"; // line 13
     *         }
     *     }
     * }
     */
    private fun generateBranchLogic(): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, "test/e2e/BranchLogic", null, "java/lang/Object", null)

        val init = cw.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null)
        init.visitCode()
        init.visitVarInsn(Opcodes.ALOAD, 0)
        init.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false)
        init.visitInsn(Opcodes.RETURN)
        init.visitMaxs(1, 1)
        init.visitEnd()

        val m = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "classify", "(I)Ljava/lang/String;", null, null)
        m.visitCode()

        val l10 = Label()
        val elseLabel = Label()
        val l11 = Label()
        val l13 = Label()

        m.visitLabel(l10)
        m.visitLineNumber(10, l10)
        m.visitVarInsn(Opcodes.ILOAD, 0)
        m.visitJumpInsn(Opcodes.IFLE, elseLabel)

        m.visitLabel(l11)
        m.visitLineNumber(11, l11)
        m.visitLdcInsn("positive")
        m.visitInsn(Opcodes.ARETURN)

        m.visitLabel(elseLabel)
        m.visitLineNumber(13, elseLabel)
        m.visitLdcInsn("non-positive")
        m.visitInsn(Opcodes.ARETURN)

        m.visitMaxs(1, 1)
        m.visitEnd()

        cw.visitEnd()
        return cw.toByteArray()
    }

    /**
     * public class test/e2e/MultiMethod {
     *     public static int first() {
     *         return 1; // line 10
     *     }
     *     public static int second() {
     *         return 2; // line 14
     *     }
     *     public static int third() {
     *         return 3; // line 18
     *     }
     * }
     */
    private fun generateMultiMethod(): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, "test/e2e/MultiMethod", null, "java/lang/Object", null)

        val init = cw.visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null)
        init.visitCode()
        init.visitVarInsn(Opcodes.ALOAD, 0)
        init.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false)
        init.visitInsn(Opcodes.RETURN)
        init.visitMaxs(1, 1)
        init.visitEnd()

        for ((name, retVal, line) in listOf(
            Triple("first", 1, 10),
            Triple("second", 2, 14),
            Triple("third", 3, 18),
        )) {
            val m = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, name, "()I", null, null)
            m.visitCode()
            val l = Label()
            m.visitLabel(l)
            m.visitLineNumber(line, l)
            m.visitLdcInsn(retVal)
            m.visitInsn(Opcodes.IRETURN)
            m.visitMaxs(1, 0)
            m.visitEnd()
        }

        cw.visitEnd()
        return cw.toByteArray()
    }

    // ---- Tests ----

    @Test
    fun `simple class - probes are hit when method is executed`() {
        val clazz = instrumentAndLoad("test/e2e/SimpleCalc", generateSimpleCalc())

        // Execute the method
        val method = clazz.getMethod("add", Int::class.java, Int::class.java)
        val result = method.invoke(null, 3, 4)
        assertEquals(7, result)

        // Verify probes were hit
        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/SimpleCalc")
        val probeData = dataStore.getData(classId)
        assertNotNull(probeData, "Data store should have entry for SimpleCalc")

        // Should have probes (at least 2: one for each line number)
        assertTrue(probeData!!.probes.size >= 2, "Should have at least 2 probes")

        // At least some probes should be hit
        assertTrue(probeData.probes.any { it }, "At least one probe should be hit after execution")
    }

    @Test
    fun `branch class - only taken branch has probes hit`() {
        val clazz = instrumentAndLoad("test/e2e/BranchLogic", generateBranchLogic())

        // Execute with positive number — should take the if branch
        val method = clazz.getMethod("classify", Int::class.java)
        val result = method.invoke(null, 5)
        assertEquals("positive", result)

        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/BranchLogic")
        val probeData = dataStore.getData(classId)
        assertNotNull(probeData, "Data store should have entry for BranchLogic")

        // Should have probes for: line 10, branch at IFLE, line 11, line 13
        // After calling classify(5), the "if" path is taken:
        //   - line 10 probe: HIT
        //   - branch probe (IFLE): HIT
        //   - line 11 probe: HIT
        //   - line 13 probe: NOT HIT
        val probes = probeData!!.probes
        assertTrue(probes.size >= 3, "Should have at least 3 probes (2 lines + 1 branch)")

        // Not all probes should be hit (else branch wasn't taken)
        assertTrue(probes.any { it }, "Some probes should be hit")
        assertTrue(probes.any { !it }, "Some probes should NOT be hit (untaken branch)")
    }

    @Test
    fun `branch class - both branches covered after two calls`() {
        val clazz = instrumentAndLoad("test/e2e/BranchLogic", generateBranchLogic())
        val method = clazz.getMethod("classify", Int::class.java)

        // Call with positive
        assertEquals("positive", method.invoke(null, 5))
        // Call with non-positive
        assertEquals("non-positive", method.invoke(null, -1))

        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/BranchLogic")
        val probeData = dataStore.getData(classId)!!

        // After both branches are taken, ALL probes should be hit
        assertTrue(probeData.probes.all { it }, "All probes should be hit after both branches taken")
    }

    @Test
    fun `multi-method class - only called methods have probes hit`() {
        val clazz = instrumentAndLoad("test/e2e/MultiMethod", generateMultiMethod())

        // Only call first() and third(), not second()
        val first = clazz.getMethod("first")
        val third = clazz.getMethod("third")

        assertEquals(1, first.invoke(null))
        assertEquals(3, third.invoke(null))

        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/MultiMethod")
        val probeData = dataStore.getData(classId)
        assertNotNull(probeData)

        // Should have 3 probes (one per method, each with 1 line)
        // The constructor also has a line, but <init> gets a probe too
        val probes = probeData!!.probes
        assertTrue(probes.size >= 3, "Should have at least 3 probes for 3 methods")

        // Some probes hit (first, third), some not (second)
        assertTrue(probes.any { it }, "Called methods should have probes hit")
        assertTrue(probes.any { !it }, "Uncalled method should have probes not hit")
    }

    @Test
    fun `multi-method class - all methods covered after full execution`() {
        val clazz = instrumentAndLoad("test/e2e/MultiMethod", generateMultiMethod())

        clazz.getMethod("first").invoke(null)
        clazz.getMethod("second").invoke(null)
        clazz.getMethod("third").invoke(null)

        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/MultiMethod")
        val probeData = dataStore.getData(classId)!!

        // After calling all methods, all probes should be hit
        // (init probe might not be hit since we use static methods)
        val hitCount = probeData.probes.count { it }
        assertTrue(hitCount >= 3, "At least 3 probes should be hit (one per method)")
    }

    @Test
    fun `instrumented class still functions correctly`() {
        // Verify instrumentation doesn't break the class behavior
        val clazz = instrumentAndLoad("test/e2e/SimpleCalc", generateSimpleCalc())
        val method = clazz.getMethod("add", Int::class.java, Int::class.java)

        assertEquals(7, method.invoke(null, 3, 4))
        assertEquals(0, method.invoke(null, -5, 5))
        assertEquals(-3, method.invoke(null, -1, -2))
        assertEquals(2_000_000_000, method.invoke(null, 1_000_000_000, 1_000_000_000))
    }

    @Test
    fun `Compose-generated class is not instrumented`() {
        val config = AgentConfig(composeFilterEnabled = true)
        val composeTransformer = OmnivoreClassTransformer(dataStore, config = config)

        val bytecode = generateSimpleCalc()
        val result = composeTransformer.transform(
            null, "com/example/ComposableSingletons\$MainKt", null, null, bytecode
        )
        assertTrue(result == null, "ComposableSingletons should not be instrumented")

        // Regular class should still be instrumented
        val result2 = composeTransformer.transform(
            null, "com/example/MyViewModel", null, null, bytecode
        )
        assertNotNull(result2, "Regular class should be instrumented")
    }

    @Test
    fun `probe data survives across multiple method calls`() {
        val clazz = instrumentAndLoad("test/e2e/SimpleCalc", generateSimpleCalc())
        val method = clazz.getMethod("add", Int::class.java, Int::class.java)

        // Call many times
        repeat(100) {
            method.invoke(null, it, it + 1)
        }

        val classId = OmnivoreClassTransformer.classNameToId("test/e2e/SimpleCalc")
        val probeData = dataStore.getData(classId)!!

        // All probes should be hit
        assertTrue(probeData.probes.all { it }, "All probes should be hit after 100 calls")
    }
}
