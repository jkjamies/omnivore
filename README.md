# Omnivore

Compose-aware code coverage platform for Android, Kotlin, KMP — and any project that produces llvm-cov, Go coverprofile, Python coverage.py, or lcov output.

Omnivore replaces JaCoCo + SonarQube with a purpose-built coverage pipeline: a Gradle plugin for instrumentation, a Rust dashboard for storage and visualization, and universal ingestion for any language.

## Components

| Component | Language | Purpose |
|---|---|---|
| [coverage-plugin](coverage-plugin/) | Kotlin | Gradle plugin + JVM agent for bytecode instrumentation |
| [dashboard](dashboard/) | Rust | REST API, SQLite storage, HTMX frontend, PR comments |
| [kmp-test-rig](test-rigs/kmp-test-rig/) | Kotlin | Multi-module KMP sample (Clean Architecture + MVI) |
| [android-test-rig](test-rigs/android-test-rig/) | Kotlin | Android sample with unit + instrumented tests (Clean Architecture + MVI) |
| [rust-test-rig](test-rigs/rust-test-rig/) | Rust | Rust workspace (native llvm-cov JSON) |
| [go-test-rig](test-rigs/go-test-rig/) | Go | Go module (native coverprofile) |
| [python-test-rig](test-rigs/python-test-rig/) | Python | Python package (native coverage.py JSON) |

## Quick Start

### 1. Run the Dashboard

```sh
cd dashboard
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run
# Dashboard at http://localhost:3000
```

Or create a `.env` file in the dashboard directory:

```env
DATABASE_URL=sqlite:omnivore.db?mode=rwc
GITHUB_TOKEN=ghp_your_token_here  # Optional: enables source code viewing
```

Then just run `cargo run`.

### 2. Add the Plugin to Your Project

#### KMP / Pure Kotlin

```kotlin
// settings.gradle.kts
pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

// build.gradle.kts (root project)
plugins {
    id("io.github.jkjamies.omnivore") version "0.1.0"
}

omnivore {
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
        markdown { enabled.set(true) }
    }
    dependencies {
        enabled.set(true)  // Dependency graph in report
    }
    dashboard {
        url.set("http://localhost:3000")
    }
}
```

For multi-module projects, apply the plugin **only at the root** — it automatically instruments all subproject test tasks and aggregates coverage data.

```kotlin
// settings.gradle.kts
include(":core")
include(":app")

// build.gradle.kts (root)
plugins {
    kotlin("jvm") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
}

subprojects {
    apply(plugin = "org.jetbrains.kotlin.jvm")
    // ...
}
```

#### Android

```kotlin
// build.gradle.kts (root project)
plugins {
    id("com.android.application") version "8.8.2" apply false
    kotlin("android") version "2.1.10" apply false
    id("io.github.jkjamies.omnivore")
}

omnivore {
    reports {
        json { enabled.set(true) }
        html { enabled.set(true) }
    }
    instrumentedTests {
        enabled.set(true)  // Enable on-device coverage collection
    }
    dashboard {
        url.set("http://localhost:3000")
    }
}
```

Instrumented test coverage requires:
- An Android emulator or device connected via ADB
- AGP 8.x+ for build-time bytecode transformation
- `OmnivoreTestListener` is automatically added to the test runner

#### Compose Filtering

Omnivore auto-detects Compose and filters out compiler-generated classes. To manually control:

```kotlin
omnivore {
    composeFilter {
        enabled.set(true)   // Auto-detected when Compose plugin is applied
    }
}
```

### 3. Run Tests and Generate Report

```sh
# Single command — triggers tests, generates report
./gradlew omnivoreReport

# Upload to dashboard
./gradlew omnivoreUpload
```

The `omnivoreReport` task automatically depends on all test tasks, so you don't need to run `test` separately.

**CLI output** shows separate sections for unit and instrumented tests with colored progress bars:

```
  Omnivore Coverage Report

  ── Unit Tests ──────────────────────────────────────  12 files

  Lines      ████████████████░░░░░░░░   68.6%  151/220
  Branches   ███████████████░░░░░░░░░   63.7%  79/124

  File                                     Lines   Branches
  ──────────────────────────────────────  ───────  ────────
  …/repository/InMemoryUserRepository.kt   63.6%   33.3%   7/11
  …/presentation/UserListStore.kt          63.3%   53.8%   31/49
  …/util/Calculator.kt                     86.4%   88.9%   19/22
  ...

  Dependencies: 2 modules, 1 edges
  Reports: build/reports/omnivore
  Formats: json, html, markdown
```

Reports are generated in `build/reports/omnivore/`:
- `omnivore-report.json` — machine-readable coverage data
- `index.html` — visual HTML report
- `coverage.md` — Markdown summary

### Non-Gradle Projects

Upload coverage from any language via curl:

```sh
# Rust / Swift (llvm-cov JSON)
cargo llvm-cov --json > coverage.json
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=llvm-cov&project_id=my-app&project_name=My+App" \
  --data-binary @coverage.json

# Go (native coverprofile)
go test -coverprofile=coverage.out ./...
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=go&project_id=my-app&project_name=My+App" \
  --data-binary @coverage.out

# Python (coverage.py JSON)
python3 -m coverage run -m pytest && python3 -m coverage json
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=python&project_id=my-app&project_name=My+App" \
  --data-binary @coverage.json

# lcov (C/C++, or any tool with lcov output)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=lcov&project_id=my-app&project_name=My+App" \
  --data-binary @coverage.lcov

# Omnivore JSON (Kotlin/Android/KMP via plugin)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage" \
  -H "Content-Type: application/json" \
  -d @omnivore-report.json
```

## Dashboard Setup

### Prerequisites

- Rust toolchain (install via [rustup](https://rustup.rs/))
- SQLite (bundled via `sqlx`)

### Running Locally

```sh
cd dashboard

# Option A: Environment variable
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run

# Option B: .env file (recommended)
echo 'DATABASE_URL=sqlite:omnivore.db?mode=rwc' > .env
cargo run
```

The dashboard starts on `http://localhost:3000`. The SQLite database is created automatically.

### Configuration

| Environment Variable | Required | Description |
|---|---|---|
| `DATABASE_URL` | Yes | SQLite connection string |
| `BIND_ADDR` | No | Listen address (default: `0.0.0.0:3000`) |
| `GITHUB_TOKEN` | No | GitHub PAT for source code viewing and PR comments |
| `OMNIVORE_DASHBOARD_URL` | No | Public URL for "View report" links in PR comments |
| `OMNIVORE_RETENTION_FULL` | No | Full snapshots to keep per project+target (default: 30) |
| `OMNIVORE_RETENTION_SUMMARY` | No | Summary-only snapshots to keep beyond full (default: 60) |

### Source Code Viewing

To see annotated source code in the dashboard:

1. Set `GITHUB_TOKEN` environment variable with a GitHub PAT that has `repo` scope
2. Configure the project's GitHub repo and source root in the dashboard settings page
3. Source code is fetched on-demand from GitHub when viewing file coverage — no source is embedded in reports

### Project Settings

After uploading coverage, configure project settings via the dashboard UI or API:

```sh
# Link to GitHub repo + set source root for path mapping
curl -X PATCH "http://localhost:3000/api/v1/projects/my-project" \
  -H "Content-Type: application/json" \
  -d '{"github_repo": "owner/repo", "source_root": "app/src/main/kotlin"}'
```

The `source_root` maps JVM class paths (e.g., `com/example/MyClass.kt`) to repository paths (e.g., `app/src/main/kotlin/com/example/MyClass.kt`).

### Hosting

The dashboard is a single binary + SQLite file. Deployment options:

- **Local development**: `cargo run` with `.env`
- **Intranet**: Deploy binary behind a reverse proxy (Nginx, Caddy)
- **Cloud**: Docker container, Fly.io, Railway, or any VPS

## Features

- **Compose-aware** — filters out Compose compiler artifacts (ComposableSingletons, LiveLiterals, lambda groups)
- **Multi-module** — apply once at root, instruments all subprojects automatically
- **Separate reporting** — unit and instrumented test coverage shown as independent sections with different thresholds
- **Android instrumented tests** — build-time bytecode transform via AGP, on-device coverage collection
- **Multi-format ingestion** — omnivore JSON, llvm-cov, Go coverprofile, Python coverage.py, lcov (auto-detected)
- **Dependency graph** — resolves and visualizes module dependencies (D3.js force-directed graph)
- **PR comments** — posts coverage summary with delta to GitHub pull requests
- **Dashboard** — HTMX frontend with coverage trends (Chart.js), nested file tree, uncovered hotspots, dark/light theme toggle
- **Source code viewing** — on-demand GitHub source fetching with coverage annotations
- **Configurable thresholds** — global defaults with per-project override, gradient coverage bars
- **Coverage badges** — shields.io-style SVG badges for READMEs (`/badge/{project_id}`)
- **Project management** — tags/labels, pinning/favoriting, search/filter, sparkline trends
- **Activity log** — recent ingest history on home page and project detail pages
- **System health** — uptime, DB size, snapshot count, last ingest at `/health`
- **Data retention** — configurable full + summary snapshot retention, automatic pruning
- **Export reports** — Markdown/JSON, single snapshot or two-snapshot comparison

## Documentation

- [Gradle Plugin Details](coverage-plugin/CLAUDE.md)
- [Dashboard Architecture](dashboard/CLAUDE.md)
- [Publishing Setup](coverage-plugin/PUBLISHING-REQUIRED.md)
- [CI/CD Integration (GitHub Actions)](.github/workflows/coverage.yml)

## License

Apache-2.0
