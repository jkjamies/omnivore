package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.instrumentation.OmnivoreClassTransformer
import com.jkjamies.omnivore.agent.reporter.CoverageAnalyzer
import com.jkjamies.omnivore.agent.runtime.ExecutionDataStore
import com.jkjamies.omnivore.agent.runtime.OmnivoreRuntime
import com.jkjamies.omnivore.agent.runtime.ProbeMap
import com.jkjamies.omnivore.agent.runtime.ProbeType
import org.junit.jupiter.api.AfterEach
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNotNull
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import java.net.URI
import javax.tools.JavaFileObject
import javax.tools.SimpleJavaFileObject
import javax.tools.ToolProvider

/**
 * Verifies that branch coverage measures *which edges were taken*, not merely
 * that a condition was reached.
 *
 * This is the regression test for the central correctness defect: probes used to
 * sit in front of the conditional jump, so a test exercising only the `true`
 * path of an `if/else` reported 100% branch coverage. Reporting a metric as
 * fully satisfied when half of it was never executed is the worst possible
 * failure direction for a coverage gate, so these tests assert the *exact*
 * covered/total counts rather than a threshold.
 *
 * Each case compiles real Java with the JDK compiler, instruments the result,
 * loads and runs it, then analyses coverage — the full pipeline, not a
 * hand-built approximation of it.
 */
class BranchCoverageTest {

    private lateinit var dataStore: ExecutionDataStore
    private lateinit var probeMap: ProbeMap
    private lateinit var transformer: OmnivoreClassTransformer

    @BeforeEach
    fun setUp() {
        dataStore = ExecutionDataStore()
        probeMap = ProbeMap()
        OmnivoreRuntime.dataStoreOverride = dataStore
        transformer = OmnivoreClassTransformer(
            dataStore,
            probeMap,
            AgentConfig(composeFilterEnabled = false),
        )
    }

    @AfterEach
    fun tearDown() {
        OmnivoreRuntime.dataStoreOverride = null
    }

    @Test
    fun `taking only one side of an if reports half the branches`() {
        val target = compileAndInstrument(
            "Branchy",
            """
            public class Branchy {
                public static int classify(boolean flag) {
                    if (flag) {
                        return 1;
                    } else {
                        return 2;
                    }
                }
            }
            """.trimIndent(),
        )

        // Exercise the `true` path only.
        assertEquals(1, target.invoke("classify", true))

        val branches = branchTally()
        assertEquals(2, branches.total, "an if/else has two edges")
        assertEquals(
            1, branches.covered,
            "only the true edge ran, so exactly one edge should be covered — " +
                "reporting 2 here is the bug this test exists for",
        )
    }

    @Test
    fun `taking both sides of an if reports full branch coverage`() {
        val target = compileAndInstrument(
            "Branchy",
            """
            public class Branchy {
                public static int classify(boolean flag) {
                    if (flag) {
                        return 1;
                    } else {
                        return 2;
                    }
                }
            }
            """.trimIndent(),
        )

        target.invoke("classify", true)
        target.invoke("classify", false)

        val branches = branchTally()
        assertEquals(2, branches.total)
        assertEquals(2, branches.covered)
    }

    @Test
    fun `a switch contributes one branch per arm plus default`() {
        val target = compileAndInstrument(
            "Switchy",
            """
            public class Switchy {
                public static String name(int day) {
                    switch (day) {
                        case 0: return "sun";
                        case 1: return "mon";
                        case 2: return "tue";
                        default: return "other";
                    }
                }
            }
            """.trimIndent(),
        )

        // Only one arm runs.
        assertEquals("mon", target.invoke("name", 1))

        val branches = branchTally()
        assertEquals(
            4, branches.total,
            "three cases plus default — switches used to contribute no branches at all",
        )
        assertEquals(1, branches.covered, "only the `case 1` edge ran")
    }

    @Test
    fun `exercising every switch arm covers every branch`() {
        val target = compileAndInstrument(
            "Switchy",
            """
            public class Switchy {
                public static String name(int day) {
                    switch (day) {
                        case 0: return "sun";
                        case 1: return "mon";
                        case 2: return "tue";
                        default: return "other";
                    }
                }
            }
            """.trimIndent(),
        )

        target.invoke("name", 0)
        target.invoke("name", 1)
        target.invoke("name", 2)
        target.invoke("name", 99)

        val branches = branchTally()
        assertEquals(4, branches.total)
        assertEquals(4, branches.covered)
    }

    @Test
    fun `a loop condition reports both the continue and exit edges`() {
        val target = compileAndInstrument(
            "Looper",
            """
            public class Looper {
                public static int sum(int n) {
                    int total = 0;
                    for (int i = 0; i < n; i++) {
                        total += i;
                    }
                    return total;
                }
            }
            """.trimIndent(),
        )

        // n = 3 enters the body and eventually exits, so both edges of the
        // loop condition are taken.
        assertEquals(3, target.invoke("sum", 3))

        val branches = branchTally()
        assertEquals(2, branches.total)
        assertEquals(2, branches.covered)
    }

    @Test
    fun `a loop that never executes covers only the exit edge`() {
        val target = compileAndInstrument(
            "Looper",
            """
            public class Looper {
                public static int sum(int n) {
                    int total = 0;
                    for (int i = 0; i < n; i++) {
                        total += i;
                    }
                    return total;
                }
            }
            """.trimIndent(),
        )

        assertEquals(0, target.invoke("sum", 0))

        val branches = branchTally()
        assertEquals(2, branches.total)
        assertEquals(1, branches.covered, "the body was never entered")
    }

    @Test
    fun `instrumented code still computes the right answers`() {
        // Edge probes rewrite control flow, so the most basic property to hold
        // is that the program still behaves identically.
        val target = compileAndInstrument(
            "Calc",
            """
            public class Calc {
                public static int classify(int n) {
                    if (n < 0) return -1;
                    if (n == 0) return 0;
                    switch (n % 3) {
                        case 0: return 30;
                        case 1: return 31;
                        default: return 32;
                    }
                }
            }
            """.trimIndent(),
        )

        assertEquals(-1, target.invoke("classify", -5))
        assertEquals(0, target.invoke("classify", 0))
        assertEquals(30, target.invoke("classify", 3))
        assertEquals(31, target.invoke("classify", 4))
        assertEquals(32, target.invoke("classify", 5))
    }

    @Test
    fun `probe map and emitted probes agree on indices`() {
        // Probe indices are positional: if the counting walk and the emitting
        // visitor ever disagree, coverage lands on the wrong lines with no
        // error anywhere. Assert they line up.
        val target = compileAndInstrument(
            "Mixed",
            """
            public class Mixed {
                public static int go(int n, boolean flag) {
                    if (flag) { n++; }
                    switch (n % 2) {
                        case 0: return n;
                        default: return -n;
                    }
                }
            }
            """.trimIndent(),
        )
        target.invoke("go", 2, true)

        val classMap = probeMap.getAllClassMaps().single { it.className == "Mixed" }
        val data = dataStore.getData(classMap.classId)
        assertNotNull(data, "execution data should exist for the instrumented class")

        val probes = classMap.getProbes()
        assertEquals(
            probes.size, data!!.probes.size,
            "the probe array must be sized to exactly the number of mapped probes",
        )
        assertEquals(
            probes.map { it.probeIndex }, probes.indices.toList(),
            "probe indices must be dense and in emission order",
        )
        assertTrue(probes.any { it.type == ProbeType.LINE }, "line probes expected")
        assertTrue(probes.any { it.type == ProbeType.BRANCH }, "branch probes expected")
    }

    @Test
    fun `branch probes are grouped by decision point`() {
        val target = compileAndInstrument(
            "TwoIfs",
            """
            public class TwoIfs {
                public static int go(boolean a, boolean b) {
                    int n = 0;
                    if (a) n++;
                    if (b) n++;
                    return n;
                }
            }
            """.trimIndent(),
        )
        target.invoke("go", true, true)

        val classMap = probeMap.getAllClassMaps().single { it.className == "TwoIfs" }
        val groups = classMap.getProbes()
            .filter { it.type == ProbeType.BRANCH }
            .groupBy { it.branchGroup }

        assertEquals(2, groups.size, "two independent `if`s are two decision points")
        assertTrue(
            groups.values.all { it.size == 2 },
            "each `if` contributes exactly two edges",
        )
    }

    @Test
    fun `a file with no branches reports a zero branch rate, not full coverage`() {
        val target = compileAndInstrument(
            "Straight",
            """
            public class Straight {
                public static int twice(int n) {
                    return n * 2;
                }
            }
            """.trimIndent(),
        )
        target.invoke("twice", 21)

        val result = CoverageAnalyzer.analyze(dataStore, probeMap)
        val file = result.files.single()
        assertEquals(0L, file.branchesTotal)
        assertEquals(
            0.0, file.branchRate,
            "a branchless file used to report 1.0, which silently inflated every " +
                "aggregate it was rolled into",
        )
    }

    // -- helpers --

    private class BranchTally(val covered: Int, val total: Int)

    private fun branchTally(): BranchTally {
        var covered = 0
        var total = 0
        for (classMap in probeMap.getAllClassMaps()) {
            val data = dataStore.getData(classMap.classId) ?: continue
            for (probe in classMap.getProbes()) {
                if (probe.type != ProbeType.BRANCH) continue
                total++
                if (probe.probeIndex < data.probes.size && data.probes[probe.probeIndex]) covered++
            }
        }
        return BranchTally(covered, total)
    }

    /** A loaded, instrumented class ready to have static methods invoked on it. */
    private class Target(private val loaded: Class<*>) {
        fun invoke(method: String, vararg args: Any?): Any? {
            val m = loaded.declaredMethods.single { it.name == method }
            m.isAccessible = true
            return m.invoke(null, *args)
        }
    }

    private fun compileAndInstrument(className: String, source: String): Target {
        val original = compile(className, source)
        val instrumented = transformer.transform(
            javaClass.classLoader, className, null, null, original,
        )
        assertNotNull(instrumented, "transformer should have instrumented $className")

        val loader = object : ClassLoader(javaClass.classLoader) {
            override fun loadClass(name: String, resolve: Boolean): Class<*> {
                if (name == className) {
                    return defineClass(name, instrumented, 0, instrumented!!.size)
                        .also { if (resolve) resolveClass(it) }
                }
                return super.loadClass(name, resolve)
            }
        }
        return Target(loader.loadClass(className))
    }

    /** Compile a single Java source file in memory and return its bytecode. */
    private fun compile(className: String, source: String): ByteArray {
        val compiler = requireNotNull(ToolProvider.getSystemJavaCompiler()) {
            "no system Java compiler — tests must run on a JDK, not a JRE"
        }

        val output = mutableMapOf<String, ByteArray>()
        val fileManager = object : javax.tools.ForwardingJavaFileManager<javax.tools.JavaFileManager>(
            compiler.getStandardFileManager(null, null, null)
        ) {
            override fun getJavaFileForOutput(
                location: javax.tools.JavaFileManager.Location?,
                name: String,
                kind: JavaFileObject.Kind?,
                sibling: javax.tools.FileObject?,
            ): JavaFileObject = object : SimpleJavaFileObject(
                URI.create("mem:///$name.class"), JavaFileObject.Kind.CLASS
            ) {
                override fun openOutputStream() = object : java.io.ByteArrayOutputStream() {
                    override fun close() {
                        output[name] = toByteArray()
                        super.close()
                    }
                }
            }
        }

        val unit = object : SimpleJavaFileObject(
            URI.create("mem:///$className.java"), JavaFileObject.Kind.SOURCE
        ) {
            override fun getCharContent(ignoreEncodingErrors: Boolean): CharSequence = source
        }

        // -g keeps line numbers, which line probes depend on.
        val task = compiler.getTask(
            null, fileManager, null, listOf("-g"), null, listOf(unit),
        )
        check(task.call()) { "failed to compile test source for $className" }
        return requireNotNull(output[className]) { "no bytecode produced for $className" }
    }
}
