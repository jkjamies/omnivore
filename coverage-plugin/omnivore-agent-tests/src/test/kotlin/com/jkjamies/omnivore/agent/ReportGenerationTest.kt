package com.jkjamies.omnivore.agent

import com.jkjamies.omnivore.agent.model.CoverageTarget
import com.jkjamies.omnivore.agent.reporter.CoverageAnalyzer
import com.jkjamies.omnivore.agent.reporter.HtmlReportWriter
import com.jkjamies.omnivore.agent.reporter.JsonReportWriter
import com.jkjamies.omnivore.agent.runtime.*
import kotlinx.serialization.json.Json
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import java.io.File

/**
 * Tests for the report generation pipeline:
 * - ProbeMap write/read round-trip
 * - ExecutionData write/read round-trip
 * - CoverageAnalyzer correctness
 * - JSON and HTML report generation
 */
class ReportGenerationTest {

    @TempDir
    lateinit var tempDir: File

    // -- ProbeMap round-trip --

    @Test
    fun `ProbeMap write and read round-trip preserves data`() {
        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(1L, "com/example/Foo", "Foo.kt")
        classMap.addProbe(0, 10, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(1, 12, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(2, 12, "doStuff", "()V", ProbeType.BRANCH)

        val file = File(tempDir, "test.probes")
        ProbeMapWriter.write(file, probeMap)

        val loaded = ProbeMapReader.read(file)
        val loadedClass = loaded.getClassMap(1L)
        assertNotNull(loadedClass)
        assertEquals("com/example/Foo", loadedClass!!.className)
        assertEquals("Foo.kt", loadedClass.sourceFile)

        val probes = loadedClass.getProbes()
        assertEquals(3, probes.size)
        assertEquals(0, probes[0].probeIndex)
        assertEquals(10, probes[0].lineNumber)
        assertEquals(ProbeType.LINE, probes[0].type)
        assertEquals(2, probes[2].probeIndex)
        assertEquals(ProbeType.BRANCH, probes[2].type)
    }

    @Test
    fun `ProbeMap round-trip handles null source file`() {
        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(2L, "com/example/Bar", null)
        classMap.addProbe(0, 5, "run", "()V", ProbeType.LINE)

        val file = File(tempDir, "test2.probes")
        ProbeMapWriter.write(file, probeMap)

        val loaded = ProbeMapReader.read(file)
        val loadedClass = loaded.getClassMap(2L)
        assertNotNull(loadedClass)
        assertNull(loadedClass!!.sourceFile)
    }

    @Test
    fun `ProbeMap round-trip with multiple classes`() {
        val probeMap = ProbeMap()
        val c1 = probeMap.getOrCreateClassMap(10L, "com/a/A", "A.kt")
        c1.addProbe(0, 1, "m1", "()V", ProbeType.LINE)
        val c2 = probeMap.getOrCreateClassMap(20L, "com/b/B", "B.kt")
        c2.addProbe(0, 1, "m2", "()I", ProbeType.LINE)
        c2.addProbe(1, 3, "m2", "()I", ProbeType.BRANCH)

        val file = File(tempDir, "multi.probes")
        ProbeMapWriter.write(file, probeMap)

        val loaded = ProbeMapReader.read(file)
        assertNotNull(loaded.getClassMap(10L))
        assertNotNull(loaded.getClassMap(20L))
        assertEquals(1, loaded.getClassMap(10L)!!.getProbes().size)
        assertEquals(2, loaded.getClassMap(20L)!!.getProbes().size)
    }

    // -- ExecutionData round-trip --

    @Test
    fun `ExecutionData write and read round-trip preserves probe hits`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Foo", 5)
        probes[0] = true
        probes[2] = true
        probes[4] = true

        val file = File(tempDir, "test.omnivore")
        ExecutionDataWriter.write(file, store)

        val loaded = ExecutionDataReader.read(file)
        val data = loaded.getData(1L)
        assertNotNull(data)
        assertEquals("com/example/Foo", data!!.className)
        assertTrue(data.probes[0])
        assertFalse(data.probes[1])
        assertTrue(data.probes[2])
        assertFalse(data.probes[3])
        assertTrue(data.probes[4])
    }

    @Test
    fun `ExecutionData round-trip with many probes`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(99L, "com/example/Big", 100)
        // Set every 3rd probe
        for (i in 0 until 100 step 3) probes[i] = true

        val file = File(tempDir, "big.omnivore")
        ExecutionDataWriter.write(file, store)

        val loaded = ExecutionDataReader.read(file)
        val data = loaded.getData(99L)!!
        for (i in 0 until 100) {
            assertEquals(i % 3 == 0, data.probes[i], "Probe $i mismatch")
        }
    }

    // -- CoverageAnalyzer --

    @Test
    fun `CoverageAnalyzer produces correct per-file coverage`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Foo", 3)
        probes[0] = true  // line 10 hit
        probes[1] = false // line 12 not hit
        probes[2] = true  // line 14 hit

        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(1L, "com/example/Foo", "Foo.kt")
        classMap.addProbe(0, 10, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(1, 12, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(2, 14, "doStuff", "()V", ProbeType.LINE)

        val result = CoverageAnalyzer.analyze(store, probeMap)

        assertEquals(1, result.files.size)
        val file = result.files[0]
        assertEquals("com/example/Foo.kt", file.path)
        assertEquals(3, file.lines.size)

        // 2 of 3 lines covered
        assertEquals(2, file.lines.count { it.hitCount > 0 })
        assertEquals(10, file.lines[0].lineNumber)
        assertEquals(1L, file.lines[0].hitCount)
        assertEquals(12, file.lines[1].lineNumber)
        assertEquals(0L, file.lines[1].hitCount)
        assertEquals(14, file.lines[2].lineNumber)
        assertEquals(1L, file.lines[2].hitCount)

        // Summary
        assertEquals(2L, result.summary.linesCovered)
        assertEquals(3L, result.summary.linesTotal)
        assertTrue(result.summary.lineRate > 0.66 && result.summary.lineRate < 0.67)
    }

    @Test
    fun `CoverageAnalyzer handles branch coverage`() {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Branchy", 4)
        probes[0] = true  // line probe
        probes[1] = true  // branch true
        probes[2] = false // branch false
        probes[3] = true  // next line

        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(1L, "com/example/Branchy", "Branchy.kt")
        classMap.addProbe(0, 5, "check", "(Z)V", ProbeType.LINE)
        classMap.addProbe(1, 5, "check", "(Z)V", ProbeType.BRANCH)
        classMap.addProbe(2, 5, "check", "(Z)V", ProbeType.BRANCH)
        classMap.addProbe(3, 8, "check", "(Z)V", ProbeType.LINE)

        val result = CoverageAnalyzer.analyze(store, probeMap)

        assertEquals(1, result.files.size)
        // Both line 5 and line 8 are covered
        assertEquals(2L, result.summary.linesCovered)
        // 1 of 2 branches covered
        assertEquals(1L, result.summary.branchesCovered)
        assertEquals(2L, result.summary.branchesTotal)
        assertEquals(0.5, result.summary.branchRate)
    }

    @Test
    fun `CoverageAnalyzer handles multiple classes in same file`() {
        val store = ExecutionDataStore()
        // Two classes in same source file
        val probes1 = store.getOrCreateProbes(1L, "com/example/Outer", 2)
        probes1[0] = true
        probes1[1] = true
        val probes2 = store.getOrCreateProbes(2L, "com/example/Outer\$Inner", 1)
        probes2[0] = false

        val probeMap = ProbeMap()
        val c1 = probeMap.getOrCreateClassMap(1L, "com/example/Outer", "Outer.kt")
        c1.addProbe(0, 3, "foo", "()V", ProbeType.LINE)
        c1.addProbe(1, 5, "foo", "()V", ProbeType.LINE)
        val c2 = probeMap.getOrCreateClassMap(2L, "com/example/Outer\$Inner", "Outer.kt")
        c2.addProbe(0, 10, "bar", "()V", ProbeType.LINE)

        val result = CoverageAnalyzer.analyze(store, probeMap)

        // Both classes map to the same file
        assertEquals(1, result.files.size)
        assertEquals("com/example/Outer.kt", result.files[0].path)
        assertEquals(3, result.files[0].lines.size)
        assertEquals(2L, result.summary.linesCovered)
        assertEquals(3L, result.summary.linesTotal)
    }

    @Test
    fun `CoverageAnalyzer skips classes with no execution data`() {
        val store = ExecutionDataStore()
        // No probes created for classId 1

        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(1L, "com/example/Unloaded", "Unloaded.kt")
        classMap.addProbe(0, 5, "neverRun", "()V", ProbeType.LINE)

        val result = CoverageAnalyzer.analyze(store, probeMap)

        // Class was never loaded so no execution data exists — skipped
        assertEquals(0, result.files.size)
        assertEquals(0L, result.summary.linesTotal)
    }

    @Test
    fun `CoverageAnalyzer empty data produces empty result`() {
        val result = CoverageAnalyzer.analyze(ExecutionDataStore(), ProbeMap())
        assertEquals(0, result.files.size)
        assertEquals(0L, result.summary.linesCovered)
        assertEquals(0L, result.summary.linesTotal)
        assertEquals(0.0, result.summary.lineRate)
        assertEquals(1.0, result.summary.branchRate) // no branches = 100%
    }

    // -- JSON report --

    @Test
    fun `JsonReportWriter produces valid JSON`() {
        val analysisResult = buildSampleAnalysisResult()

        val file = File(tempDir, "report.json")
        JsonReportWriter.write(
            outputFile = file,
            analysisResult = analysisResult,
            projectId = "test-project",
            projectName = "Test Project",
            target = CoverageTarget.JVM_UNIT,
            commitSha = "abc123",
            branch = "main",
        )

        assertTrue(file.exists())
        val content = file.readText()

        // Parse to verify it's valid JSON
        val json = Json { ignoreUnknownKeys = true }
        val report = json.decodeFromString(
            com.jkjamies.omnivore.agent.model.OmnivoreReport.serializer(),
            content,
        )

        assertEquals("test-project", report.project.id)
        assertEquals("Test Project", report.project.name)
        assertEquals("abc123", report.project.commitSha)
        assertEquals("main", report.project.branch)
        assertEquals(CoverageTarget.JVM_UNIT, report.project.target)
        assertEquals(analysisResult.summary.lineRate, report.coverage.lineRate)
        assertEquals(analysisResult.files.size, report.files.size)
    }

    // -- HTML report --

    @Test
    fun `HtmlReportWriter produces valid HTML`() {
        val analysisResult = buildSampleAnalysisResult()

        val file = File(tempDir, "report.html")
        HtmlReportWriter.write(file, analysisResult)

        assertTrue(file.exists())
        val html = file.readText()
        assertTrue(html.contains("<!DOCTYPE html>"))
        assertTrue(html.contains("Omnivore Coverage Report"))
        assertTrue(html.contains("com/example/Foo.kt"))
        // Verify the summary values are present
        assertTrue(html.contains("66.7%") || html.contains("66.6%"))
    }

    @Test
    fun `HtmlReportWriter escapes HTML in file paths`() {
        val analysisResult = CoverageAnalyzer.AnalysisResult(
            files = listOf(
                com.jkjamies.omnivore.agent.model.FileCoverage(
                    path = "com/<script>alert(1)</script>/Evil.kt",
                    lineRate = 1.0,
                    branchRate = 1.0,
                    lines = listOf(com.jkjamies.omnivore.agent.model.LineCoverage(1, 1L)),
                )
            ),
            summary = com.jkjamies.omnivore.agent.model.CoverageSummary(
                lineRate = 1.0, branchRate = 1.0,
                linesCovered = 1, linesTotal = 1,
                branchesCovered = 0, branchesTotal = 0,
            ),
        )

        val file = File(tempDir, "evil.html")
        HtmlReportWriter.write(file, analysisResult)

        val html = file.readText()
        assertFalse(html.contains("<script>"))
        assertTrue(html.contains("&lt;script&gt;"))
    }

    // -- Full pipeline: instrument → execute → analyze → report --

    @Test
    fun `full pipeline from execution data through report`() {
        // Simulate what happens after tests run:
        // 1. Write execution data + probe map
        // 2. Read them back
        // 3. Analyze
        // 4. Generate reports

        // Step 1: Create execution data and probe map (simulating agent output)
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(42L, "com/app/Service", 4)
        probes[0] = true
        probes[1] = true
        probes[2] = false
        probes[3] = true

        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(42L, "com/app/Service", "Service.kt")
        classMap.addProbe(0, 10, "handle", "(Ljava/lang/String;)V", ProbeType.LINE)
        classMap.addProbe(1, 11, "handle", "(Ljava/lang/String;)V", ProbeType.LINE)
        classMap.addProbe(2, 13, "handle", "(Ljava/lang/String;)V", ProbeType.LINE)
        classMap.addProbe(3, 15, "handle", "(Ljava/lang/String;)V", ProbeType.LINE)

        val execFile = File(tempDir, "coverage.omnivore")
        val probeFile = File(tempDir, "coverage.probes")
        ExecutionDataWriter.write(execFile, store)
        ProbeMapWriter.write(probeFile, probeMap)

        // Step 2: Read them back (simulating reporter)
        val loadedStore = ExecutionDataReader.read(execFile)
        val loadedProbeMap = ProbeMapReader.read(probeFile)

        // Step 3: Analyze
        val result = CoverageAnalyzer.analyze(loadedStore, loadedProbeMap)

        assertEquals(1, result.files.size)
        assertEquals(3L, result.summary.linesCovered)
        assertEquals(4L, result.summary.linesTotal)
        assertEquals(0.75, result.summary.lineRate)

        // Step 4: Generate reports
        val jsonFile = File(tempDir, "report.json")
        val htmlFile = File(tempDir, "report.html")
        JsonReportWriter.write(jsonFile, result, "my-app", "My App")
        HtmlReportWriter.write(htmlFile, result)

        assertTrue(jsonFile.exists())
        assertTrue(htmlFile.exists())
        assertTrue(jsonFile.length() > 0)
        assertTrue(htmlFile.length() > 0)
    }

    // -- Helper --

    private fun buildSampleAnalysisResult(): CoverageAnalyzer.AnalysisResult {
        val store = ExecutionDataStore()
        val probes = store.getOrCreateProbes(1L, "com/example/Foo", 3)
        probes[0] = true
        probes[1] = false
        probes[2] = true

        val probeMap = ProbeMap()
        val classMap = probeMap.getOrCreateClassMap(1L, "com/example/Foo", "Foo.kt")
        classMap.addProbe(0, 10, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(1, 12, "doStuff", "()V", ProbeType.LINE)
        classMap.addProbe(2, 14, "doStuff", "()V", ProbeType.LINE)

        return CoverageAnalyzer.analyze(store, probeMap)
    }
}
