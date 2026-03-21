# go-test-rig

Go module for testing Omnivore dashboard ingestion of Go coverprofile format coverage data.

## Structure

```
model/           Domain model (Task, Priority, TaskError)
repository/      TaskRepository interface + InMemoryTaskRepository
usecase/         Business logic (AddTask, ToggleTask, GetTasks, GetStats)
validation/      Input validation (ValidateTitle, ValidateTag, SanitizeDescription)
formatter/       Text formatting (FormatTaskLine, FormatStats)
```

## Build & Test

```sh
go test ./...
```

## Coverage

Uses Go's native coverprofile format — no conversion tools needed. The dashboard ingests coverprofile directly via `?format=go`.

```sh
# Generate coverprofile and upload to dashboard
go test -coverprofile=coverage.out ./...
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=go&project_id=go-test-rig&project_name=Go+Test+Rig" \
  --data-binary @coverage.out
```

## Domain Model

Task management system (mirrors the Android/Rust test rig pattern):
- **model**: Task struct, Priority enum, error constructors
- **repository**: TaskRepository interface + in-memory implementation
- **usecase**: AddTask, ToggleTask, RemoveTask, GetTasks, GetTasksByPriority, GetStats
- **validation**: ValidateTitle, ValidateTag, SanitizeDescription
- **formatter**: FormatTaskLine, FormatTaskList, FormatStats, FormatPriorityDistribution

Intentional coverage gaps in: Update, Remove, FindByTag, NextID, ToggleTask, RemoveTask, GetTasksByPriority, FormatPriorityDistribution.
