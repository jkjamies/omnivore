# Omnivore

Compose-aware code coverage platform for Android, Kotlin, KMP — and any project that produces lcov or llvm-cov output.

Omnivore replaces JaCoCo + SonarQube with a purpose-built coverage pipeline: a Gradle plugin for instrumentation, a Rust dashboard for storage and visualization, and universal ingestion for any language.

## Components

| Component | Language | Purpose |
|---|---|---|
| [coverage-plugin](coverage-plugin/) | Kotlin | Gradle plugin + JVM agent for bytecode instrumentation |
| [dashboard](dashboard/) | Rust | REST API, SQLite storage, HTMX frontend, PR comments |
| [test-rig](test-rig/) | Kotlin | Sample project for testing the plugin |

## Quick Start

### 1. Run the Dashboard

```sh
cd dashboard
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run
# Dashboard at http://localhost:3000
```

### 2. Add the Plugin to Your Project

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

omnivore {
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
    }
    dashboard {
        url.set("http://localhost:3000")
    }
}
```

### 3. Run Tests and Upload

```sh
./gradlew test omnivoreReport omnivoreUpload
```

### Non-Gradle Projects

Upload coverage from any language via curl:

```sh
# lcov (Go, C/C++)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=lcov&project_id=my-app&project_name=My+App" \
  -d @coverage.lcov

# llvm-cov (Rust, Swift)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=llvm-cov&project_id=my-app&project_name=My+App" \
  -d @llvm-cov-export.json

# Omnivore JSON (direct)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage" \
  -H "Content-Type: application/json" \
  -d @omnivore-report.json
```

## Features

- **Compose-aware** — filters out Compose compiler artifacts (ComposableSingletons, LiveLiterals, lambda groups)
- **Android instrumented tests** — build-time bytecode transform via AGP, on-device coverage collection
- **Multi-format ingestion** — omnivore JSON, lcov, llvm-cov export (auto-detected)
- **Dependency graph** — resolves and visualizes module dependencies (D3.js force-directed graph)
- **PR comments** — posts coverage summary with delta to GitHub pull requests
- **Dashboard** — HTMX frontend with coverage trends (Chart.js), file breakdown, dark/light theme

## Documentation

- [Gradle Plugin Integration](coverage-plugin/README.md)
- [Dashboard Setup](dashboard/README.md)
- [CI/CD Integration (GitHub Actions)](docs/github-actions.md)
- [lcov Integration (Go, C/C++)](docs/lcov-integration.md)
- [llvm-cov Integration (Rust, Swift)](docs/llvm-cov-integration.md)
- [Publishing Setup](coverage-plugin/PUBLISHING-REQUIRED.md)
- [Future: Multi-Platform Dependency Graphs](FUTURE-DEPENDENCY-GRAPHS.md)

## License

Apache-2.0
