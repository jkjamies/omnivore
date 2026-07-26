package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.BuildTimeInstrumentingVisitor
import com.jkjamies.omnivore.agent.instrumentation.LoaderAwareClassWriter
import com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer
import com.jkjamies.omnivore.agent.instrumentation.ProbeInserter
import com.jkjamies.omnivore.agent.runtime.ClassId
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeEntry
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertFalse
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import org.objectweb.asm.ClassReader
import org.objectweb.asm.ClassVisitor
import org.objectweb.asm.ClassWriter
import org.objectweb.asm.FieldVisitor
import org.objectweb.asm.Label
import org.objectweb.asm.Opcodes

/**
 * Tests for the build-time (Android/AGP) instrumentation path.
 *
 * The AGP transform is the one code path that no test could previously reach:
 * it lived in the Gradle plugin module, which is `compileOnly` against AGP, so
 * exercising it needed a full Android toolchain. It was therefore also the path
 * that quietly drifted — it had its own probe walker that ignored `switch`
 * statements, and it sized every probe array at a hardcoded 1024.
 *
 * The logic now lives in [BuildTimeInstrumentingVisitor] in the agent module,
 * and these tests hold it to the load-time agent's behaviour: **same input class
 * in, same probe map out**. A divergence between the two paths means Android and
 * JVM coverage disagree about what a given probe index refers to, which is
 * exactly the failure mode that produces plausible-looking wrong numbers.
 */
class BuildTimeInstrumentationTest {

    private lateinit var dataStore: ExecutionDataStore

    @BeforeEach
    fun setUp() {
        dataStore = ExecutionDataStore()
        OmnivoreRuntime.dataStoreOverride = dataStore
    }

    @AfterEach
    fun tearDown() {
        OmnivoreRuntime.dataStoreOverride = null
    }

    private val config = AgentConfig(composeFilterEnabled = false)

    // ---- Harness ----

    private class ByteClassLoader(
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

    /** Run [bytecode] through the build-time (AGP) path. */
    private fun instrumentAtBuildTime(
        internalName: String,
        bytecode: ByteArray,
        probeMap: ProbeMap? = null,
    ): ByteArray {
        val writer = LoaderAwareClassWriter(javaClass.classLoader)
        val visitor = BuildTimeInstrumentingVisitor(internalName, config, probeMap, writer)
        ClassReader(bytecode).accept(visitor, 0)
        return writer.toByteArray()
    }

    /** Run [bytecode] through the load-time (JVM agent) path. */
    private fun instrumentAtLoadTime(
        internalName: String,
        bytecode: ByteArray,
        probeMap: ProbeMap? = null,
    ): ByteArray {
        val transformer = OmnivoreClassTransformer(dataStore, probeMap, config)
        return transformer.transform(null, internalName, null, null, bytecode)
            ?: error("$internalName was not instrumented by the load-time path")
    }

    private fun load(internalName: String, bytecode: ByteArray): Class<*> {
        val dotName = internalName.replace('/', '.')
        return ByteClassLoader(dotName, bytecode, javaClass.classLoader).loadClass(dotName)
    }

    private fun probesOf(probeMap: ProbeMap, internalName: String): List<ProbeEntry> =
        probeMap.getClassMap(ClassId.forClassName(internalName))?.getProbes().orEmpty()

    private fun fieldNames(bytecode: ByteArray): List<String> {
        val names = mutableListOf<String>()
        ClassReader(bytecode).accept(
            object : ClassVisitor(Opcodes.ASM9) {
                override fun visitField(
                    access: Int,
                    name: String?,
                    descriptor: String?,
                    signature: String?,
                    value: Any?,
                ): FieldVisitor? {
                    if (name != null) names += name
                    return null
                }
            },
            0,
        )
        return names
    }

    // ---- Fixtures ----

    private fun ClassWriter.defaultConstructor() {
        val init = visitMethod(Opcodes.ACC_PUBLIC, "<init>", "()V", null, null)
        init.visitCode()
        init.visitVarInsn(Opcodes.ALOAD, 0)
        init.visitMethodInsn(Opcodes.INVOKESPECIAL, "java/lang/Object", "<init>", "()V", false)
        init.visitInsn(Opcodes.RETURN)
        init.visitMaxs(1, 1)
        init.visitEnd()
    }

    /**
     * A class with an `if`, a `tableswitch`, and several lines — everything the
     * two paths have to agree about.
     */
    private fun generateBranchyClass(internalName: String): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, internalName, null, "java/lang/Object", null)
        cw.visitSource("Branchy.kt", null)
        cw.defaultConstructor()

        val sign = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "sign", "(I)I", null, null)
        sign.visitCode()
        val l10 = Label()
        val negative = Label()
        sign.visitLabel(l10)
        sign.visitLineNumber(10, l10)
        sign.visitVarInsn(Opcodes.ILOAD, 0)
        sign.visitJumpInsn(Opcodes.IFLT, negative)
        val l11 = Label()
        sign.visitLabel(l11)
        sign.visitLineNumber(11, l11)
        sign.visitInsn(Opcodes.ICONST_1)
        sign.visitInsn(Opcodes.IRETURN)
        sign.visitLabel(negative)
        sign.visitLineNumber(13, negative)
        sign.visitInsn(Opcodes.ICONST_M1)
        sign.visitInsn(Opcodes.IRETURN)
        sign.visitMaxs(2, 1)
        sign.visitEnd()

        val pick = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "pick", "(I)I", null, null)
        pick.visitCode()
        val l20 = Label()
        pick.visitLabel(l20)
        pick.visitLineNumber(20, l20)
        val case0 = Label()
        val case1 = Label()
        val case2 = Label()
        val default = Label()
        pick.visitVarInsn(Opcodes.ILOAD, 0)
        pick.visitTableSwitchInsn(0, 2, default, case0, case1, case2)
        for ((label, line, value) in listOf(
            Triple(case0, 21, 100),
            Triple(case1, 22, 200),
            Triple(case2, 23, 300),
            Triple(default, 24, -1),
        )) {
            pick.visitLabel(label)
            pick.visitLineNumber(line, label)
            pick.visitLdcInsn(value)
            pick.visitInsn(Opcodes.IRETURN)
        }
        pick.visitMaxs(2, 1)
        pick.visitEnd()

        cw.visitEnd()
        return cw.toByteArray()
    }

    /**
     * A class with an existing `<clinit>` and more probes than the old hardcoded
     * array size, so instrumentation has to both prepend to the static
     * initialiser and size the array from the real count.
     */
    private fun generateManyLineClass(internalName: String, lineCount: Int): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, internalName, null, "java/lang/Object", null)
        cw.visitField(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "SEED", "I", null, null).visitEnd()
        cw.defaultConstructor()

        val clinit = cw.visitMethod(Opcodes.ACC_STATIC, "<clinit>", "()V", null, null)
        clinit.visitCode()
        clinit.visitIntInsn(Opcodes.BIPUSH, 7)
        clinit.visitFieldInsn(Opcodes.PUTSTATIC, internalName, "SEED", "I")
        clinit.visitInsn(Opcodes.RETURN)
        clinit.visitMaxs(1, 0)
        clinit.visitEnd()

        val count = cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_STATIC, "count", "(I)I", null, null)
        count.visitCode()
        for (i in 0 until lineCount) {
            val label = Label()
            count.visitLabel(label)
            count.visitLineNumber(i + 1, label)
            count.visitVarInsn(Opcodes.ILOAD, 0)
            count.visitInsn(Opcodes.ICONST_1)
            count.visitInsn(Opcodes.IADD)
            count.visitVarInsn(Opcodes.ISTORE, 0)
        }
        val last = Label()
        count.visitLabel(last)
        count.visitLineNumber(lineCount + 1, last)
        count.visitVarInsn(Opcodes.ILOAD, 0)
        count.visitInsn(Opcodes.IRETURN)
        count.visitMaxs(2, 1)
        count.visitEnd()

        cw.visitEnd()
        return cw.toByteArray()
    }

    private fun generateInterface(internalName: String): ByteArray {
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(
            Opcodes.V17,
            Opcodes.ACC_PUBLIC or Opcodes.ACC_ABSTRACT or Opcodes.ACC_INTERFACE,
            internalName,
            null,
            "java/lang/Object",
            null,
        )
        cw.visitMethod(Opcodes.ACC_PUBLIC or Opcodes.ACC_ABSTRACT, "apply", "()I", null, null).visitEnd()
        cw.visitEnd()
        return cw.toByteArray()
    }

    // ---- Tests ----

    @Test
    fun `build-time and load-time paths produce identical probe maps`() {
        val name = "test/buildtime/Branchy"
        val bytecode = generateBranchyClass(name)

        val buildTimeMap = ProbeMap()
        instrumentAtBuildTime(name, bytecode, buildTimeMap)

        val loadTimeMap = ProbeMap()
        instrumentAtLoadTime(name, bytecode, loadTimeMap)

        val fromBuild = probesOf(buildTimeMap, name)
        val fromLoad = probesOf(loadTimeMap, name)

        assertTrue(fromBuild.isNotEmpty(), "build-time path recorded no probes")
        assertEquals(
            fromLoad,
            fromBuild,
            "Android and JVM instrumentation disagree about probe indices; " +
                "the same index would mean different source lines on each platform",
        )
    }

    @Test
    fun `build-time path records both edges of a conditional and every switch arm`() {
        val name = "test/buildtime/BranchyEdges"
        val probeMap = ProbeMap()
        instrumentAtBuildTime(name, generateBranchyClass(name), probeMap)

        val branches = probesOf(probeMap, name).filter { it.type == ProbeType.BRANCH }

        val sign = branches.filter { it.methodName == "sign" }
        assertEquals(
            ProbeInserter.BRANCH_PROBES_PER_JUMP,
            sign.size,
            "a conditional jump needs one probe per outgoing edge",
        )
        assertEquals(1, sign.map { it.branchGroup }.toSet().size, "both edges belong to one decision")

        // The old AGP-side walker had no switch handling at all, so these were
        // silently absent from Android reports.
        val pick = branches.filter { it.methodName == "pick" }
        assertEquals(
            ProbeInserter.switchProbeCount(3),
            pick.size,
            "a 3-case tableswitch needs a probe per case plus default",
        )
        assertEquals(1, pick.map { it.branchGroup }.toSet().size)
        assertTrue(
            sign.first().branchGroup != pick.first().branchGroup ||
                sign.first().methodName != pick.first().methodName,
            "branch groups are scoped per method",
        )
    }

    @Test
    fun `a class with more probes than the old fixed array still runs`() {
        // The previous build-time implementation emitted
        // `getProbes(classId, name, 1024)` for any class with a <clinit>, so
        // probe 1024 wrote past the end of the array and threw inside the app
        // under test. Edge probes roughly doubled probe counts, which was about
        // to start making this reachable in ordinary code.
        val name = "test/buildtime/ManyLines"
        val lineCount = 1_200
        val probeMap = ProbeMap()
        val instrumented = instrumentAtBuildTime(name, generateManyLineClass(name, lineCount), probeMap)

        val expectedProbes = lineCount + 1
        assertEquals(expectedProbes, probesOf(probeMap, name).size)

        val clazz = load(name, instrumented)
        // Loading runs <clinit>: the existing one must still assign SEED, and the
        // prepended prologue must allocate an array of the real size.
        assertEquals(7, clazz.getField("SEED").getInt(null))

        val result = clazz.getMethod("count", Int::class.java).invoke(null, 0)
        assertEquals(lineCount, result)

        val data = dataStore.getData(ClassId.forClassName(name))
        assertNotNull(data, "probe array should have been registered from <clinit>")
        assertEquals(expectedProbes, data!!.probes.size)
        assertTrue(data.probes.all { it }, "every line ran, so every probe should be set")
    }

    @Test
    fun `build-time instrumented class behaves the same as the original`() {
        val name = "test/buildtime/BranchyBehaviour"
        val clazz = load(name, instrumentAtBuildTime(name, generateBranchyClass(name)))

        val sign = clazz.getMethod("sign", Int::class.java)
        assertEquals(1, sign.invoke(null, 4))
        assertEquals(1, sign.invoke(null, 0))
        assertEquals(-1, sign.invoke(null, -4))

        val pick = clazz.getMethod("pick", Int::class.java)
        assertEquals(100, pick.invoke(null, 0))
        assertEquals(200, pick.invoke(null, 1))
        assertEquals(300, pick.invoke(null, 2))
        assertEquals(-1, pick.invoke(null, 9))
    }

    @Test
    fun `only the executed edge of a conditional is recorded`() {
        val name = "test/buildtime/BranchyEdgeHits"
        val probeMap = ProbeMap()
        val clazz = load(name, instrumentAtBuildTime(name, generateBranchyClass(name), probeMap))

        assertEquals(1, clazz.getMethod("sign", Int::class.java).invoke(null, 4))

        val probes = dataStore.getData(ClassId.forClassName(name))!!.probes
        val signBranches = probesOf(probeMap, name)
            .filter { it.methodName == "sign" && it.type == ProbeType.BRANCH }

        assertEquals(ProbeInserter.BRANCH_PROBES_PER_JUMP, signBranches.size)
        assertEquals(
            1,
            signBranches.count { probes[it.probeIndex] },
            "taking one path must set exactly one of the two edge probes",
        )
    }

    @Test
    fun `interfaces pass through without a probe field`() {
        val name = "test/buildtime/Marker"
        val probeMap = ProbeMap()
        val instrumented = instrumentAtBuildTime(name, generateInterface(name), probeMap)

        assertFalse(
            fieldNames(instrumented).contains(ProbeInserter.PROBE_FIELD_NAME),
            "an interface cannot carry the ACC_TRANSIENT probe field",
        )
        assertTrue(probesOf(probeMap, name).isEmpty())

        // Still a loadable interface.
        assertTrue(load(name, instrumented).isInterface)
    }

    @Test
    fun `a class with no instrumentable code is passed through unchanged`() {
        val name = "test/buildtime/Empty"
        val cw = ClassWriter(ClassWriter.COMPUTE_FRAMES)
        cw.visit(Opcodes.V17, Opcodes.ACC_PUBLIC, name, null, "java/lang/Object", null)
        cw.visitEnd()
        val original = cw.toByteArray()

        val probeMap = ProbeMap()
        val instrumented = instrumentAtBuildTime(name, original, probeMap)

        assertFalse(fieldNames(instrumented).contains(ProbeInserter.PROBE_FIELD_NAME))
        assertTrue(probesOf(probeMap, name).isEmpty())
    }

    @Test
    fun `instrumenting twice records each probe once`() {
        // The build-time visitor both builds the probe map and replays the class
        // through ProbeInserter. If it also let ProbeInserter record, every probe
        // would be listed twice and branch totals would double.
        val name = "test/buildtime/NoDoubleCount"
        val probeMap = ProbeMap()
        instrumentAtBuildTime(name, generateBranchyClass(name), probeMap)

        val entries = probesOf(probeMap, name)
        assertEquals(
            entries.size,
            entries.map { it.probeIndex }.toSet().size,
            "probe indices must be unique — a duplicate means the probe map was written twice",
        )
    }
}
