# coverage-plugin

Multi-module Gradle build providing a JVM agent for bytecode instrumentation and a Gradle plugin for integration.

## Modules

```
omnivore-agent/          JVM agent (fat JAR) — instrumentation, runtime, reporting
omnivore-gradle-plugin/  Gradle plugin — test task configuration, report task
omnivore-agent-tests/    Integration tests for the agent
```

## Build & Test

```sh
./gradlew build    # Build all modules
./gradlew test     # Run all tests
```

## Key Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| ASM | 9.9.1 | Bytecode analysis & transformation (core, tree, commons, util) |
| kotlinx-serialization | 1.11.0 | JSON report generation |
| AGP | 8.8.2 | Android Gradle Plugin integration (compileOnly) |
| Kotlin | 2.3.21 | Language version |
| JUnit 4 | 4.13.2 | `RunListener` for Android instrumented tests (compileOnly in agent) |
| JUnit 5 | 6.0.3 | Testing |
| Java toolchain | 17 | Target JVM |

Version catalog: `gradle/libs.versions.toml`

## Architecture

### Agent (`omnivore-agent`)

**Entry point:** `OmnivoreAgent.kt` — two modes:
- `premain()` for JVM agent (`-javaagent`) — unit tests
- `initialize()` for direct bootstrapping — Android instrumented tests (called by `OmnivoreTestListener`)

**Instrumentation pipeline** (`OmnivoreClassTransformer`):
1. Filter infrastructure classes (JDK, Kotlin stdlib, test frameworks, Android, Compose libs)
2. Check include/exclude patterns (glob-based)
3. Compose-aware filtering via `ComposeDetector` (ComposableSingletons, LiveLiterals, Composer params, lambda groups)
4. Kotlin-aware filtering via `KotlinDetector` (synthetic bridges, data class methods, coroutine continuations)
5. First pass: analyze with ASM tree API, count probes
6. Second pass: instrument with `InstrumentingClassVisitor` + `ProbeInserter`

**Probe system:**
- Each class gets a static `$omnivoreProbes: BooleanArray` field
- `<clinit>` calls `OmnivoreRuntime.getProbes(classId, className, probeCount)`
- `ExecutionDataStore` holds all probe arrays (thread-safe, concurrent)
- Class IDs are CRC64 of the class *name* (`ClassId`) — not the bytecode as
  JaCoCo uses, because the ID is baked into the instrumented `<clinit>` and the
  AGP path never sees the original bytes

**Probe placement — branch probes are on control-flow EDGES, not on the branch
instruction.** A probe in front of a conditional jump fires when the condition is
*evaluated*, which says nothing about which way control went; that made
`if (x) a() else b()` report 100% branch coverage from a test taking only the
`true` path. Each conditional jump is rewritten to route both outcomes through
their own probe block, and each switch arm (plus `default`) gets a probe.

**The invariant that matters:** probe indices are positional, so
`ClassInstrumenter.walkProbes` must stay in lockstep with `ProbeInserter`'s
emission order. If they diverge, coverage is attributed to the wrong source lines
with no error anywhere. Counting, probe-map building, and instrumentation all go
through that one traversal for exactly this reason — and both the JVM agent and
the AGP transform share it (`ClassInstrumenter` / `InstrumentingClassVisitor`),
rather than keeping the parallel implementations that had already drifted apart.

**Diagnostics:** `InstrumentationStats` counts why classes were or weren't
instrumented; the agent writes a `.stats` file next to the `.omnivore` data and
`omnivoreReport` prints a summary line. A class can drop out of coverage for
several unrelated reasons, all of which were previously a silent `return null`.

**Shutdown:** `ShutdownHook` flushes `.omnivore` + `.probes` files on JVM exit.

**Reporting:**
- `CoverageAnalyzer` correlates execution data with probe maps
- Writers: `JsonReportWriter`, `HtmlReportWriter`, `MarkdownReportWriter`

### Plugin (`omnivore-gradle-plugin`)

**Entry point:** `OmnivorePlugin.kt` — applies to `Project`, registers extension + tasks.

**DSL** (`OmnivoreExtension`):
```kotlin
omnivore {
    includes.set(listOf("com.example.*"))
    excludes.set(listOf("com.example.generated.*"))
    composeFilter { enabled.set(true) }
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
        markdown { enabled.set(false) }
    }
    dashboard {
        url.set("http://localhost:3000")
        projectId.set("my-project")
    }
}
```

**UnitTestConfigurator:** Wires `-javaagent` to all `Test` tasks with config from extension.

**InstrumentedTestConfigurator:** For Android projects with `instrumentedTests.enabled = true`:
- Adds slim runtime JAR as `implementation` dependency eagerly via `plugins.withId()` (before AGP resolves configurations)
- 3-tier JAR resolution: included build → Gradle configuration → fat JAR extraction (JaCoCo-inspired)
- Registers AGP build-time bytecode transform via `OmnivoreClassVisitorFactory` using `plugins.withId()` reactive pattern
- Configures test runner arguments (`listener`, `omnivore.destdir`, `omnivore.compose`)
- Registers `omnivoreWriteBuildProbeMap` task — writes probe map accumulated during ASM transform
- Registers `omnivoreSetupDevice` task — creates writable directory on device before tests
- Registers `omnivorePullCoverage` task (finalizer) — extracts coverage from logcat (primary), with fallback to adb pull and run-as
- Coverage data lands in `build/omnivore/connectedAndroidTest/`

**OmnivoreTestListener** (`com.jkjamies.omnivore.agent.android`): JUnit 4 `RunListener` that initializes the agent on `testRunStarted` and flushes `.omnivore`/`.probes` files on `testRunFinished`. Outputs coverage data as base64 via System.err (logcat) with marker lines — this bypasses Android SELinux restrictions on `/data/local/tmp/` and survives AGP's post-test app uninstallation.

**OmnivoreClassVisitorFactory** (`com.jkjamies.omnivore.gradle.transform`): AGP `AsmClassVisitorFactory` that applies probe instrumentation at build time (Android has no `-javaagent` support).

**OmnivoreReportTask:** Scans `build/omnivore/` for `.omnivore` + `.probes` files (from both unit and instrumented tests), analyzes each target (`JVM_UNIT`, `ANDROID_INSTRUMENTED`) independently, and writes one `omnivore-report.json` per target to `build/reports/omnivore/` — top-level for a single target, or under a target-named subdirectory (`jvm-unit/`, `android-instrumented/`) when multiple targets are present, so each uploads as its own dashboard series. Local `index.html`/`coverage.md` use a merged combined view.

**OmnivoreUploadTask:** Walks `build/reports/omnivore/` and POSTs each `omnivore-report.json` (one per target) to the dashboard's ingestion endpoint.

**DependencyGraphResolver** (`com.jkjamies.omnivore.gradle.configuration`): Walks Gradle's resolved configurations (`runtimeClasspath`, `testRuntimeClasspath`) to build a graph of modules and edges. Supports internal project modules and optionally external (Maven) dependencies.

**DSL config:**
```kotlin
omnivore {
    dependencies {
        enabled.set(true)              // Include dependency graph in report
        includeExternal.set(false)     // Include third-party deps
        includeTestDeps.set(false)     // Include test-scoped deps
    }
}
```

## Binary Formats

**`.omnivore` (execution data):** Magic `OMNIVORE` (8 bytes) + version (short) + class entries (classId: long, className: UTF, probes: bit-packed booleans).

**`.probes` (probe maps):** Magic `OMNIPROB` (8 bytes) + version (short) + class entries with probe metadata (index, line, method, descriptor, type: LINE/BRANCH, isComposable, branchGroup).

Format **v3**. v1/v2 files are rejected rather than upgraded: branch probes moved
onto control-flow edges, so probe indices mean something different and pairing an
old `.probes` with new execution data would silently attribute coverage to the
wrong lines. `branchGroup` identifies which decision point an edge belongs to, so
two edges of one `if` are distinguishable from two unrelated branches.

**`.stats` (instrumentation summary):** Java `Properties` — instrumented count,
per-reason skip counts, and up to 20 failure messages.

## Publishing

License: **Apache-2.0**. Both modules publish to Maven Central (OSSRH) and the plugin also to the Gradle Plugin Portal.

- `maven-publish` + `signing` plugins on both `omnivore-agent` and `omnivore-gradle-plugin`
- GPG signing via in-memory PGP keys (env vars `GPG_SIGNING_KEY`, `GPG_SIGNING_PASSWORD`)
- OSSRH credentials via env vars (`OSSRH_USERNAME`, `OSSRH_PASSWORD`) or gradle properties
- Signing is required for non-SNAPSHOT versions
- Plugin Portal metadata (website, vcsUrl, tags) configured in `gradlePlugin {}` block
- See `PUBLISHING-REQUIRED.md` for the full setup checklist
