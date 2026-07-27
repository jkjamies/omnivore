package com.jkjamies.omnivore.agent.instrumentation

/**
 * Shared matcher for the include/exclude patterns in the Omnivore DSL.
 *
 * Patterns are globs by default (`*` = any run of characters, `?` = one
 * character); prefix a pattern with `regex:` to supply a regular expression
 * instead.
 *
 * ## Why this exists
 *
 * The glob-to-regex conversion was previously duplicated in three places and
 * only escaped `.`:
 *
 * ```kotlin
 * pattern.replace(".", "\\.").replace("*", ".*").replace("?", ".")
 * ```
 *
 * Every other regex metacharacter passed through untouched, so a pattern that
 * mentioned one was quietly reinterpreted rather than matched literally. The
 * case that bites in practice is `$`, which appears in the name of every JVM
 * inner and synthetic class: excluding `com.example.Foo$Bar` compiled to a
 * regex meaning "`com.example.Foo` then end-of-input, then `Bar`", which
 * matches nothing — so the exclusion silently did not apply and the class kept
 * being instrumented. `+`, `(`, `)`, `[`, `]`, `{`, `}`, `|`, and `^` had the
 * same problem, with `(` and `[` throwing outright on an unbalanced pattern.
 *
 * Compiled patterns are cached: these are evaluated per class during
 * instrumentation, which is a hot path.
 */
object GlobPattern {

    private const val REGEX_PREFIX = "regex:"

    private val cache = java.util.concurrent.ConcurrentHashMap<String, Regex?>()

    /** Match [text] against [pattern]. An invalid `regex:` pattern never matches. */
    fun matches(pattern: String, text: String): Boolean =
        compile(pattern)?.matches(text) ?: false

    private fun compile(pattern: String): Regex? = cache.computeIfAbsent(pattern) {
        try {
            if (it.startsWith(REGEX_PREFIX)) {
                Regex(it.removePrefix(REGEX_PREFIX))
            } else {
                Regex(globToRegex(it))
            }
        } catch (e: Exception) {
            System.err.println("[Omnivore] Ignoring invalid pattern '$it': ${e.message}")
            null
        }
    }

    /**
     * Translate a glob to a regex, escaping every character that is not a
     * wildcard so it matches literally.
     */
    internal fun globToRegex(glob: String): String = buildString(glob.length * 2) {
        for (c in glob) {
            when (c) {
                '*' -> append(".*")
                '?' -> append('.')
                // Escape anything with meaning to the regex engine. Letters,
                // digits and characters like '_' or '/' need no escaping.
                '.', '\\', '+', '(', ')', '[', ']', '{', '}', '|', '^', '$' -> {
                    append('\\').append(c)
                }
                else -> append(c)
            }
        }
    }
}
