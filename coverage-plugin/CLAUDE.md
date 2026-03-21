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
| ASM | 9.7.1 | Bytecode analysis & transformation (core, tree, commons, util) |
| kotlinx-serialization | 1.8.0 | JSON report generation |
| AGP | 8.8.2 | Android Gradle Plugin integration (compileOnly) |
| Kotlin | 2.1.10 | Language version |
| JUnit 4 | 4.13.2 | `RunListener` for Android instrumented tests (compileOnly in agent) |
| JUnit 5 | 5.11.4 | Testing |
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
- `ProbeInserter` sets `probes[index] = true` at line/branch points
- `ExecutionDataStore` holds all probe arrays (thread-safe, concurrent)

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

**OmnivoreReportTask:** Scans `build/omnivore/` for `.omnivore` + `.probes` files (from both unit and instrumented tests), merges, analyzes, writes reports to `build/reports/omnivore/`. Auto-detects coverage target: `COMPOSITE` if both unit and instrumented data present, `ANDROID_INSTRUMENTED` if only instrumented, `JVM_UNIT` otherwise.

**OmnivoreUploadTask:** POSTs `omnivore-report.json` to the dashboard's ingestion endpoint.

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

**`.probes` (probe maps):** Magic `OMNIPROB` (8 bytes) + version (short) + class entries with probe metadata (index, line, method, descriptor, type: LINE/BRANCH).

## Publishing

License: **Apache-2.0**. Both modules publish to Maven Central (OSSRH) and the plugin also to the Gradle Plugin Portal.

- `maven-publish` + `signing` plugins on both `omnivore-agent` and `omnivore-gradle-plugin`
- GPG signing via in-memory PGP keys (env vars `GPG_SIGNING_KEY`, `GPG_SIGNING_PASSWORD`)
- OSSRH credentials via env vars (`OSSRH_USERNAME`, `OSSRH_PASSWORD`) or gradle properties
- Signing is required for non-SNAPSHOT versions
- Plugin Portal metadata (website, vcsUrl, tags) configured in `gradlePlugin {}` block
- See `PUBLISHING-REQUIRED.md` for the full setup checklist
