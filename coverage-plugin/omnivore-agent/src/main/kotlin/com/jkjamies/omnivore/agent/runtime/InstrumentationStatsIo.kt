package com.jkjamies.omnivore.agent.runtime

import com.jkjamies.omnivore.agent.instrumentation.InstrumentationStats
import java.io.File
import java.util.Properties

/**
 * Persists [InstrumentationStats] so the Gradle side can report them.
 *
 * Instrumentation happens in the forked test JVM, so the counters are gone by
 * the time `omnivoreReport` runs in the daemon. Writing a small `.stats` file
 * next to the `.omnivore` data is what lets the report task print
 * "instrumented 412 classes, skipped 38 (…)" — the signal whose absence let a
 * whole category of silent instrumentation failures go unnoticed.
 *
 * Plain `Properties` on purpose: this is diagnostic output, it must never be
 * the reason a build fails, and a human should be able to read it.
 */
object InstrumentationStatsIo {

    const val EXTENSION = "stats"

    private const val KEY_INSTRUMENTED = "instrumented"
    private const val KEY_SKIP_PREFIX = "skipped."
    private const val KEY_FAILURE_PREFIX = "failure."

    fun write(file: File) {
        try {
            val props = Properties()
            props.setProperty(KEY_INSTRUMENTED, InstrumentationStats.instrumentedClasses().toString())
            for (reason in InstrumentationStats.SkipReason.entries) {
                val count = InstrumentationStats.skippedCount(reason)
                if (count > 0) props.setProperty("$KEY_SKIP_PREFIX${reason.name}", count.toString())
            }
            // Cap the failure list: a systemic problem produces one entry per
            // class, and the first handful are enough to diagnose it.
            for ((className, message) in InstrumentationStats.failureDetails().entries.take(20)) {
                props.setProperty("$KEY_FAILURE_PREFIX$className", message)
            }

            file.parentFile?.mkdirs()
            file.outputStream().use { props.store(it, "Omnivore instrumentation summary") }
        } catch (e: Exception) {
            System.err.println("[Omnivore] Could not write instrumentation stats: ${e.message}")
        }
    }

    /** Read one `.stats` file; returns null if it is missing or unreadable. */
    fun read(file: File): Summary? = try {
        val props = Properties()
        file.inputStream().use { props.load(it) }

        val skipped = mutableMapOf<String, Int>()
        val failures = mutableMapOf<String, String>()
        for (name in props.stringPropertyNames()) {
            when {
                name.startsWith(KEY_SKIP_PREFIX) ->
                    skipped[name.removePrefix(KEY_SKIP_PREFIX)] =
                        props.getProperty(name).toIntOrNull() ?: 0
                name.startsWith(KEY_FAILURE_PREFIX) ->
                    failures[name.removePrefix(KEY_FAILURE_PREFIX)] = props.getProperty(name)
            }
        }

        Summary(
            instrumented = props.getProperty(KEY_INSTRUMENTED)?.toIntOrNull() ?: 0,
            skippedByReason = skipped,
            failures = failures,
        )
    } catch (_: Exception) {
        null
    }

    data class Summary(
        val instrumented: Int,
        val skippedByReason: Map<String, Int>,
        val failures: Map<String, String>,
    ) {
        val totalSkipped: Int get() = skippedByReason.values.sum()

        operator fun plus(other: Summary) = Summary(
            instrumented = instrumented + other.instrumented,
            skippedByReason = (skippedByReason.keys + other.skippedByReason.keys).associateWith {
                (skippedByReason[it] ?: 0) + (other.skippedByReason[it] ?: 0)
            },
            failures = failures + other.failures,
        )

        /** Reasons worth surfacing, most common first. */
        fun describeSkips(): String = skippedByReason.entries
            .filter { it.value > 0 }
            .sortedByDescending { it.value }
            .joinToString(", ") { "${it.value} ${it.key.lowercase().replace('_', ' ')}" }
    }
}
