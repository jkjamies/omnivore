# Omnivore Gradle Plugin

Compose-aware code coverage for Android, Kotlin, and KMP projects.

## Installation

### From Gradle Plugin Portal

```kotlin
// build.gradle.kts
plugins {
    id("io.github.jkjamies.omnivore") version "0.1.0"
}
```

### From Maven Central (manual)

```kotlin
// settings.gradle.kts
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

// build.gradle.kts
plugins {
    id("io.github.jkjamies.omnivore") version "0.1.0"
}
```

### From Local Source (development)

```kotlin
// settings.gradle.kts
pluginManagement {
    includeBuild("../coverage-plugin")
}

// build.gradle.kts
plugins {
    id("io.github.jkjamies.omnivore")
}
```

## Configuration

```kotlin
omnivore {
    // Package patterns (empty = instrument everything)
    includes.set(listOf("com.example.*"))
    excludes.set(listOf("com.example.generated.*"))

    // Compose bytecode filtering (auto-detected if Compose plugin is applied)
    composeFilter {
        enabled.set(true)
        additionalExcludePatterns.set(listOf("com.example.ui.preview.*"))
    }

    // Android instrumented test coverage
    instrumentedTests {
        enabled.set(true)
    }

    // Report formats
    reports {
        json { enabled.set(true) }   // omnivore-report.json (for dashboard)
        html { enabled.set(true) }   // index.html (standalone)
        markdown { enabled.set(true) } // coverage.md
    }

    // Dependency graph
    dependencies {
        enabled.set(true)
        includeExternal.set(false)   // Include third-party dependencies
        includeTestDeps.set(false)   // Include test-scoped dependencies
    }

    // Dashboard upload
    dashboard {
        url.set("http://localhost:3000")
        apiKey.set(providers.environmentVariable("OMNIVORE_API_KEY"))
        projectId.set("my-project")  // Defaults to project.name
    }
}
```

## Tasks

| Task | Description |
|---|---|
| `omnivoreReport` | Generate coverage reports from test execution data |
| `omnivoreUpload` | Upload `omnivore-report.json` to the dashboard |

## Usage

### Basic: Run Tests and Generate Report

```sh
./gradlew test
./gradlew omnivoreReport
```

Reports are generated in `build/reports/omnivore/`:
- `omnivore-report.json` — machine-readable (for dashboard ingestion)
- `index.html` — standalone HTML report
- `coverage.md` — Markdown summary

### Upload to Dashboard

```sh
./gradlew omnivoreUpload -Pomnivore.dashboard.url=http://localhost:3000
```

Or configure the URL in the DSL (see above).

### Android Instrumented Tests

Enable in the DSL:

```kotlin
omnivore {
    instrumentedTests {
        enabled.set(true)
    }
}
```

The plugin will:
1. Add the agent JAR as an `androidTestImplementation` dependency
2. Register an AGP bytecode transform (`OmnivoreClassVisitorFactory`) for build-time instrumentation
3. Configure `OmnivoreTestListener` as the JUnit 4 RunListener
4. Register `omnivorePullCoverage` to pull `.omnivore`/`.probes` files from the device after tests

Run connected tests, then generate the report:

```sh
./gradlew connectedDebugAndroidTest
./gradlew omnivorePullCoverage
./gradlew omnivoreReport
```

### Dependency Graph

Enable to include a module dependency graph in the JSON report:

```kotlin
omnivore {
    dependencies {
        enabled.set(true)
    }
}
```

The graph is embedded in `omnivore-report.json` and visualized on the dashboard at `/projects/{id}/dependencies`.

Options:
- `includeExternal.set(true)` — include third-party Maven dependencies
- `includeTestDeps.set(true)` — include test-scoped dependencies

## How It Works

1. **Unit tests**: The plugin attaches a JVM agent (`-javaagent:omnivore-agent.jar`) to `Test` tasks. The agent instruments classes at load time, inserting boolean probes at line and branch points.

2. **Android tests**: Since ART doesn't support `-javaagent`, the plugin registers an AGP `AsmClassVisitorFactory` for build-time instrumentation. A JUnit 4 `RunListener` initializes the agent and flushes coverage data on the device.

3. **Data collection**: During test execution, probes are set to `true` when code is reached. On JVM shutdown (or `RunListener.testRunFinished`), the agent writes `.omnivore` (execution data) and `.probes` (probe maps) files.

4. **Report generation**: `omnivoreReport` reads all `.omnivore` + `.probes` files, merges them, analyzes coverage, and writes reports.

## Requirements

- Gradle 8.0+
- JDK 17+
- Kotlin 2.0+ (for Compose filtering)
- AGP 8.0+ (for Android instrumented tests)

## License

Apache-2.0
