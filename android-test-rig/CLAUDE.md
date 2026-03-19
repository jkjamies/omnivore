# android-test-rig

Android project for testing the Omnivore coverage plugin with both unit and instrumented tests. Uses MVI + Clean Architecture.

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

Uses composite build (`includeBuild("../coverage-plugin")`) to resolve the plugin from local source. Both unit and instrumented test coverage are enabled. Compose filter is disabled.

## Architecture (Clean Architecture + MVI)

```
domain/
  model/         Task data class with Priority enum
  repository/    TaskRepository interface
  usecase/       GetTasks, AddTask, ToggleTask, DeleteTask use cases

data/
  repository/    InMemoryTaskRepository implementation

presentation/
  TaskListContract.kt   MVI State, Intent, Effect definitions
  TaskListViewModel.kt  ViewModel processing intents → state + effects
```

## Source Files & Intentional Gaps

| Layer | File | Untested |
|---|---|---|
| Domain | `Task.kt` | `isOverdue()`, description search |
| Domain | `GetTasksUseCase.kt` | `COMPLETED` filter |
| Domain | `AddTaskUseCase.kt` | Title > 200 chars validation |
| Domain | `ToggleTaskUseCase.kt` | Fully untested |
| Domain | `DeleteTaskUseCase.kt` | Fully untested |
| Data | `InMemoryTaskRepository.kt` | `updateTask()`, `getTasksByPriority()` |
| Presentation | `TaskListViewModel.kt` | Toggle, delete, add, setFilter, effects |
| Presentation | `TaskListContract.kt` | `activeCount`, `completedCount` |
