# android-test-rig

Multi-module Android project for testing the Omnivore coverage plugin with both unit and instrumented tests. Uses Clean Architecture + MVI.

## How to Run

```sh
# Requires coverage-plugin to be built first and a running emulator/device.
# Runs unit tests + connected Android tests + generates combined report.
./gradlew clean omnivoreReport

# Upload report to dashboard (requires running dashboard)
./gradlew omnivoreUpload -Pomnivore.dashboard.url=http://localhost:3000
```

Reports generated in `build/reports/omnivore/`:
- `omnivore-report.json` — machine-readable coverage data. When a run has both
  unit and instrumented coverage, one is written per target under a target-named
  subdirectory (`jvm-unit/omnivore-report.json`,
  `android-instrumented/omnivore-report.json`) so the dashboard tracks each as a
  separate series; a unit-only run writes a single top-level `omnivore-report.json`.
  `omnivoreUpload` posts each of them.
- `index.html` — visual HTML report (combined view)
- `coverage.md` — Markdown summary (combined view)

### JaCoCo (opt-in second coverage source)

The `:app` module can also emit a JaCoCo XML report via AGP's bundled JaCoCo —
a convenient way to exercise the dashboard's JaCoCo/Kover ingestion path with a
real Android tool. It is **opt-in** via the `omnivore.jacoco` property, kept off
by default because AGP's unit-test coverage instruments the same tests the
Omnivore agent already covers during `omnivoreReport`; running one at a time
avoids interference. (The KMP rig has the equivalent wired up with Kover.)

```sh
# Generate a JaCoCo XML report for the app module's unit tests
./gradlew :app:createDebugUnitTestCoverageReport -Pomnivore.jacoco
# → app/build/reports/coverage/test/debug/report.xml  (path may vary by AGP version;
#   otherwise: find app/build -name 'report.xml' -path '*coverage*')

# Ingest it — recorded as target=JVM_UNIT, source=jacoco (a separate series from
# the omnivore-agent report, so both trend independently on the dashboard)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=jacoco&project_id=android-test-rig&project_name=Android+Test+Rig" \
  --data-binary @app/build/reports/coverage/test/debug/report.xml
```

Without `-Pomnivore.jacoco`, unit-test coverage stays disabled and the standard
`omnivoreReport` flow (and CI) is completely unaffected.

## Modules

```
:domain    Pure Kotlin — model, repository interface, use cases
:data      Pure Kotlin — InMemoryTaskRepository (depends on :domain)
:common    Pure Kotlin — validation + formatting utilities (depends on :domain)
:app       Android — presentation layer only (depends on :domain, :data, :common)
```

Dependency graph: `common → domain`, `data → domain`, `app → domain/data/common`

## Plugin Configuration

Uses composite build (`includeBuild("../../coverage-plugin")`) in both `pluginManagement` (for plugin resolution) and at root level (for dependency resolution). Both unit and instrumented test coverage are enabled. Compose filter is disabled. Dependency graph collection is enabled.

## Instrumented Test Flow

`omnivoreReport` drives the full pipeline:

1. **Build-time instrumentation**: AGP ASM transform (`OmnivoreClassVisitorFactory`) instruments app classes with coverage probes before dexing
2. **Device setup**: `omnivoreSetupDevice` task creates `/data/local/tmp/omnivore/` on the device
3. **Connected tests**: `connectedDebugAndroidTest` runs 5 tests on emulator/device
4. **Coverage extraction**: `OmnivoreTestListener` flushes coverage data and outputs it as base64 via logcat (System.err). This bypasses Android's SELinux restrictions on `/data/local/tmp/` (shell_data_file context) and survives AGP's post-test app uninstallation.
5. **Pull**: `omnivorePullCoverage` (finalizer) parses logcat output, base64-decodes coverage files, validates magic headers
6. **Report**: `omnivoreReport` merges unit test + instrumented test data into a combined report with separate sections

## Architecture (Clean Architecture + MVI)

```
domain/
  model/         Task data class with Priority enum
  repository/    TaskRepository interface
  usecase/       GetTasks, AddTask, ToggleTask, DeleteTask use cases

data/
  repository/    InMemoryTaskRepository implementation

common/
  validation/    TaskValidator — title/description validation, sanitization
  formatting/    TaskFormatter — task line, list, summary, priority breakdown

app/
  presentation/
    TaskListContract.kt   MVI State, Intent, Effect definitions
    TaskListViewModel.kt  ViewModel processing intents → state + effects
```

## Source Files & Intentional Gaps

| Module | File | Untested |
|---|---|---|
| domain | `Task.kt` | `isOverdue()`, description search |
| domain | `GetTasksUseCase.kt` | `COMPLETED` filter |
| domain | `AddTaskUseCase.kt` | Title > 200 chars validation |
| domain | `ToggleTaskUseCase.kt` | Fully untested |
| domain | `DeleteTaskUseCase.kt` | Fully untested |
| data | `InMemoryTaskRepository.kt` | `updateTask()`, `getTasksByPriority()` |
| common | `TaskValidator.kt` | `validateDescription()`, `validateTask()`, `sanitizeDescription()`, title > 200 |
| common | `TaskFormatter.kt` | `formatPriorityBreakdown()`, `formatSummary()` empty, multi-task list |
| app | `TaskListViewModel.kt` | Toggle, delete, add, setFilter, effects |
| app | `TaskListContract.kt` | `activeCount`, `completedCount` |
