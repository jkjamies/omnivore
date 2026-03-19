# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Omnivore — compose-aware code coverage platform replacing JaCoCo + SonarQube for Android, Kotlin, and KMP projects. Also ingests lcov and llvm-cov formats for any language.

## Architecture

```
coverage-plugin/     Gradle plugin + JVM agent (Kotlin, multi-module Gradle build)
dashboard/           REST API + HTMX frontend (Rust workspace, Axum + Askama + SQLite)
kmp-test-rig/        Multi-module KMP test project (unit tests, dependency graph)
android-test-rig/    Android test project (unit + instrumented tests)
schema/              Shared data format definitions (planned)
.github/workflows/   CI (coverage.yml) and publish (publish.yml) workflows
```

Each sub-project has its own `CLAUDE.md` with detailed architecture, build commands, and conventions. See those for component-specific guidance.

**Data flow:** Plugin instruments bytecode (JVM agent) → agent collects probe data during tests → `omnivoreReport` task generates `omnivore-report.json` → `omnivoreUpload` task POSTs to dashboard → dashboard ingests via REST API → stores in SQLite → HTMX frontend displays trends + file breakdown.

## Quick Reference

### Build Everything

```sh
# Plugin (Gradle 8.12, Kotlin 2.1.10, Java 17)
cd coverage-plugin && ./gradlew build

# Dashboard (Rust 2024 edition — DATABASE_URL required for sqlx compile-time checks)
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo build

# KMP test rig (requires plugin build first — uses composite build)
cd kmp-test-rig && ./gradlew test omnivoreReport

# Android test rig (requires plugin build first — uses composite build)
cd android-test-rig && ./gradlew test omnivoreReport
```

### Run Tests

```sh
# Plugin — all tests
cd coverage-plugin && ./gradlew test

# Plugin — single test class or method
cd coverage-plugin && ./gradlew :omnivore-agent-tests:test --tests ComposeDetectorTest
cd coverage-plugin && ./gradlew :omnivore-agent-tests:test --tests "*.ComposeDetectorTest.testMethodName"

# Dashboard — all tests
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo test

# Dashboard — single test
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo test test_ingest_omnivore_format
```

### Run Dashboard Locally

```sh
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run
# Starts on http://localhost:3000
```

### End-to-End Flow (test-rig → dashboard)

```sh
# 1. Start dashboard (separate terminal)
cd dashboard && DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run

# 2. Build plugin, run tests, generate report, upload (KMP)
cd kmp-test-rig && ./gradlew test omnivoreReport omnivoreUpload

# 3. Or use Android test rig
cd android-test-rig && ./gradlew test omnivoreReport omnivoreUpload
```

## Conventions

- Project name: **Omnivore** (everywhere)
- License: **Apache-2.0**
- Version: `0.1.0-SNAPSHOT` (plugin), `0.1.0` (dashboard)
- Gradle plugin ID: `io.github.jkjamies.omnivore`
- Group ID: `io.github.jkjamies`
- Report format: `omnivore-report.json` — camelCase fields, kotlinx-serialization (Kotlin) ↔ serde (Rust)
- Coverage targets: `JVM_UNIT`, `ANDROID_INSTRUMENTED`, `IOS_UNIT`, `KOTLIN_NATIVE`, `COMPOSITE`

## CI/CD

- **`coverage.yml`** — push to `main` + PRs: build kmp-test-rig, generate report, upload to dashboard
- **`publish.yml`** — `v*` tags: publish agent + plugin to Maven Central (OSSRH) + Gradle Plugin Portal
- See `coverage-plugin/PUBLISHING-REQUIRED.md` for one-time setup checklist