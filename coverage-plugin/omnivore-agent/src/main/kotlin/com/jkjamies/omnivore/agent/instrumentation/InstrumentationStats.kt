package com.jkjamies.omnivore.agent.instrumentation

import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicInteger

/**
 * Counts why classes were or were not instrumented, so the outcome is visible
 * instead of inferred.
 *
 * A class can drop out of coverage for several independent reasons — it was
 * filtered, it had no line numbers, its classloader could not see the runtime,
 * or instrumentation threw — and every one of those paths used to end in a
 * quiet `return null`, with at most a line on stderr. That is how a bug where
 * ASM could not resolve application types (and therefore skipped every
 * branch-heavy class) stayed invisible: the numbers were simply lower than they
 * should have been, with nothing to suggest anything had failed.
 *
 * The report task prints a one-line summary from these counters. "Instrumented
 * 412 classes, skipped 38 (12 no line numbers, 26 filtered)" makes the next
 * such bug obvious on the first run.
 */
object InstrumentationStats {

    /** Why a class was not instrumented. */
    enum class SkipReason {
        /** Filtered by the built-in infrastructure prefix list. */
        INFRASTRUCTURE,

        /** Excluded by user include/exclude patterns or annotations. */
        FILTERED,

        /** Compose-generated or Compose-filtered. */
        COMPOSE,

        /** The classloader could not see OmnivoreRuntime. */
        NO_RUNTIME_ACCESS,

        /** Came from a test source set. */
        TEST_SOURCES,

        /** An interface — cannot carry the probe field. */
        INTERFACE,

        /**
         * No probes to insert. Almost always a class compiled without debug
         * information, since line probes come from the line-number table.
         */
        NO_LINE_NUMBERS,

        /** Already instrumented at build time by the AGP transform. */
        ALREADY_INSTRUMENTED,

        /** Instrumentation threw. */
        ERROR,
    }

    private val instrumentedCount = AtomicInteger()
    private val skipped = ConcurrentHashMap<SkipReason, AtomicInteger>()
    private val failures = ConcurrentHashMap<String, String>()

    fun instrumented() {
        instrumentedCount.incrementAndGet()
    }

    fun skipped(reason: SkipReason) {
        skipped.computeIfAbsent(reason) { AtomicInteger() }.incrementAndGet()
    }

    fun failed(className: String, error: Throwable) {
        skipped(SkipReason.ERROR)
        // Keep the first failure per class; a flood of identical messages is
        // noise, but the class names are what a user needs.
        failures.putIfAbsent(className, error.toString())
    }

    fun instrumentedClasses(): Int = instrumentedCount.get()

    fun skippedCount(reason: SkipReason): Int = skipped[reason]?.get() ?: 0

    fun totalSkipped(): Int = skipped.values.sumOf { it.get() }

    /** Class name to error message for classes that failed to instrument. */
    fun failureDetails(): Map<String, String> = failures.toMap()

    /**
     * A human-readable one-liner, or null when nothing was instrumented at all
     * (in which case the caller has a more urgent message to print).
     */
    fun summary(): String? {
        val instrumented = instrumentedCount.get()
        val total = totalSkipped()
        if (instrumented == 0 && total == 0) return null

        val breakdown = SkipReason.entries
            .mapNotNull { reason ->
                val count = skippedCount(reason)
                if (count > 0) "$count ${reason.describe()}" else null
            }
            .joinToString(", ")

        return buildString {
            append("Instrumented $instrumented class(es)")
            if (total > 0) append(", skipped $total ($breakdown)")
        }
    }

    fun reset() {
        instrumentedCount.set(0)
        skipped.clear()
        failures.clear()
    }

    private fun SkipReason.describe(): String = when (this) {
        SkipReason.INFRASTRUCTURE -> "infrastructure"
        SkipReason.FILTERED -> "filtered"
        SkipReason.COMPOSE -> "Compose"
        SkipReason.NO_RUNTIME_ACCESS -> "runtime not visible"
        SkipReason.TEST_SOURCES -> "test sources"
        SkipReason.INTERFACE -> "interfaces"
        SkipReason.NO_LINE_NUMBERS -> "no line numbers"
        SkipReason.ALREADY_INSTRUMENTED -> "already instrumented"
        SkipReason.ERROR -> "errors"
    }
}
