# test-rig

Sample Kotlin project for testing the Omnivore coverage plugin. Contains intentionally partial test coverage to exercise reporting.

## How to Run

```sh
# Requires coverage-plugin to be built first
./gradlew test omnivoreReport

# Upload report to dashboard (requires running dashboard)
./gradlew omnivoreUpload -Pomnivore.dashboard.url=http://localhost:3000
```

Reports generated in `build/reports/omnivore/`:
- `omnivore-report.json` — machine-readable coverage data
- `index.html` — visual HTML report
- `coverage.md` — Markdown summary

## Plugin Configuration

Uses composite build (`includeBuild("../coverage-plugin")`) to resolve the plugin from local source. Compose filter is disabled. JSON, HTML, and Markdown reports are enabled.

Dashboard URL is configured via `providers.gradleProperty("omnivore.dashboard.url")` with a default of `http://localhost:3000`.

Dependency graph collection is enabled (internal modules only by default). Add `includeExternal.set(true)` and `includeTestDeps.set(true)` to see external/test dependencies.

## Source Files

| File | Purpose | Intentional gaps |
|---|---|---|
| `Calculator.kt` | Math ops + branching | `multiply()` untested, medium/large branches untested |
| `StringUtils.kt` | String manipulation | `toCamelCase()`, `countWords()` untested, edge cases skipped |
| `UserService.kt` | CRUD with `User` data class | `deactivateUser()`, `removeUser()`, `findByEmail()` untested |

## Expected Coverage

~78% line coverage, ~71% branch coverage. Per-file rates vary from ~50% (StringUtils) to ~100% (test files themselves).
