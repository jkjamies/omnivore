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
- `omnivore-report.json` — machine-readable coverage data
- `index.html` — visual HTML report
- `coverage.md` — Markdown summary

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
