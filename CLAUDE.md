# Omnivore

Compose-aware code coverage platform replacing JaCoCo + SonarQube for Android, Kotlin, and KMP projects.

## Architecture

```
coverage-plugin/     Gradle plugin + JVM agent (Kotlin, multi-module Gradle build)
dashboard/           REST API + HTMX frontend (Rust workspace, Axum + Askama)
test-rig/            Sample project for testing the plugin
schema/              Shared data format definitions (planned)
.github/workflows/   CI (coverage.yml) and publish (publish.yml) workflows
```

**Data flow:** Plugin instruments bytecode → agent collects probe data during tests → report task generates `omnivore-report.json` → `omnivoreUpload` task POSTs to dashboard → dashboard ingests via REST API → stores in SQLite → HTMX frontend displays trends + file breakdown.

## Conventions

- Project name: **Omnivore** (everywhere)
- License: **Apache-2.0**
- Version: `0.1.0-SNAPSHOT` (plugin), `0.1.0` (dashboard)
- Gradle plugin ID: `io.github.jkjamies.omnivore`
- Group ID: `io.github.jkjamies` (free via GitHub username verification)
- Binary formats: `.omnivore` (execution data), `.probes` (probe maps)
- Report format: `omnivore-report.json` (kotlinx-serialization ↔ serde, camelCase field names)

## Building

```sh
# Plugin + agent
cd coverage-plugin && ./gradlew build

# Dashboard (needs DATABASE_URL for sqlx compile-time checks)
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo build

# Test rig (requires plugin build first)
cd test-rig && ./gradlew test omnivoreReport
```

## CI/CD

- **`coverage.yml`** — runs on push to `main`: build, test, generate report, upload to dashboard, save artifacts
- **`publish.yml`** — runs on `v*` tags: publish agent to Maven Central (OSSRH), plugin to Maven Central + Gradle Plugin Portal
- See `coverage-plugin/PUBLISHING-REQUIRED.md` for one-time setup checklist

## Component Interaction

1. `coverage-plugin` instruments bytecode (JVM agent for unit tests, AGP transform for Android)
2. Agent collects probe data during test execution
3. `omnivoreReport` task merges unit + instrumented coverage, generates `omnivore-report.json`
4. `omnivoreUpload` task POSTs report to `POST /api/v1/ingest/coverage`
5. Dashboard auto-creates projects, stores snapshots in SQLite
6. HTMX frontend shows project list, coverage trends (Chart.js), and file breakdown
7. If PR metadata is provided, dashboard posts a Markdown coverage comment to the GitHub PR

## Report Format (OmnivoreReport)

Top-level fields: `version`, `format` ("omnivore"), `project` (id, name, commitSha, branch, target), `coverage` (lineRate, branchRate, counts), `files` (per-file line-level coverage).

Coverage targets: `JVM_UNIT`, `ANDROID_INSTRUMENTED`, `IOS_UNIT`, `KOTLIN_NATIVE`, `COMPOSITE`.

## Dependency Graph

The plugin can optionally resolve and embed a dependency graph in the report. The dashboard stores it and provides a D3.js force-directed visualization at `/projects/{id}/dependencies`.

DSL: `omnivore { dependencies { enabled.set(true); includeExternal.set(true); includeTestDeps.set(false) } }`

API: `GET /api/v1/coverage/{project_id}/dependencies`

See `FUTURE-DEPENDENCY-GRAPHS.md` for multi-platform extension plans (Rust/Go/Swift/etc.).

## Multi-Platform Ingestion

The dashboard accepts coverage data in three formats via `POST /api/v1/ingest/coverage`:
- **Omnivore JSON** — native format from the Gradle plugin (auto-detected)
- **lcov** — `go test -coverprofile`, gcov/lcov (`?format=lcov&project_id=...&project_name=...`)
- **llvm-cov export JSON** — `cargo llvm-cov --json`, Xcode (`?format=llvm-cov&project_id=...`)

Format is auto-detected from content, or specified via `?format=` query parameter. Non-omnivore formats require project metadata via query params since they don't carry it natively.
