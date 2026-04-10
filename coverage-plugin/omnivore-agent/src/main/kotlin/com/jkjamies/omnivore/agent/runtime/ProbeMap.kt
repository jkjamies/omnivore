package com.jkjamies.omnivore.agent.runtime

import java.util.concurrent.ConcurrentHashMap

/**
 * Maps probe indices back to source file locations.
 *
 * During instrumentation, each probe is associated with a source file,
 * line number, and method. This map is written alongside execution data
 * so that the reporter can generate meaningful coverage reports.
 */
class ProbeMap {

    private val entries = ConcurrentHashMap<Long, ClassProbeMap>()

    fun getOrCreateClassMap(classId: Long, className: String, sourceFile: String?): ClassProbeMap {
        return entries.computeIfAbsent(classId) {
            ClassProbeMap(classId, className, sourceFile)
        }
    }

    fun getClassMap(classId: Long): ClassProbeMap? = entries[classId]

    fun getAllClassMaps(): Collection<ClassProbeMap> = entries.values.toList()

    fun removeClassMap(classId: Long) {
        entries.remove(classId)
    }

    fun isEmpty(): Boolean = entries.isEmpty()
}

/**
 * Probe mapping for a single class.
 */
class ClassProbeMap(
    val classId: Long,
    val className: String,
    val sourceFile: String?,
) {
    private val probeEntries = mutableListOf<ProbeEntry>()

    @Synchronized
    fun addProbe(probeIndex: Int, lineNumber: Int, methodName: String, methodDesc: String, type: ProbeType, isComposable: Boolean = false) {
        probeEntries.add(ProbeEntry(probeIndex, lineNumber, methodName, methodDesc, type, isComposable))
    }

    @Synchronized
    fun removeProbe(probeIndex: Int) {
        probeEntries.removeAll { it.probeIndex == probeIndex }
    }

    @Synchronized
    fun getProbes(): List<ProbeEntry> = probeEntries.toList()

    /**
     * Get all unique line numbers that have probes.
     */
    fun getCoveredLineNumbers(): Set<Int> {
        return probeEntries.filter { it.lineNumber > 0 }.map { it.lineNumber }.toSet()
    }

    /**
     * Returns true if ALL instrumentable methods in this class are @Composable.
     * Used to auto-exclude pure Compose files from JVM unit test coverage.
     */
    fun isAllMethodsComposable(): Boolean {
        if (probeEntries.isEmpty()) return false
        val methods = probeEntries.map { it.methodName to it.methodDesc }.toSet()
        return methods.all { (name, desc) ->
            probeEntries.any { it.methodName == name && it.methodDesc == desc && it.isComposable }
        }
    }
}

data class ProbeEntry(
    val probeIndex: Int,
    val lineNumber: Int,
    val methodName: String,
    val methodDesc: String,
    val type: ProbeType,
    val isComposable: Boolean = false,
)

enum class ProbeType {
    LINE,
    BRANCH,
}
