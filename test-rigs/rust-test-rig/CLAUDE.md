# rust-test-rig

Rust workspace for testing Omnivore dashboard ingestion of `llvm-cov` format coverage data.

## Structure

```
core/    Library — domain model (Task, Priority), repository trait, use cases, validation
app/     Library — re-exports core + formatter module
```

## Build & Test

```sh
cargo test
```

## Coverage

Uses `cargo-llvm-cov` — no wrapper scripts needed. The dashboard ingests llvm-cov JSON directly.

```sh
cargo install cargo-llvm-cov

# Generate llvm-cov JSON and upload to dashboard
cargo llvm-cov --json > coverage.json
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=llvm-cov&project_id=rust-test-rig&project_name=Rust+Test+Rig" \
  --data-binary @coverage.json
```

If using Homebrew Rust (no rustup), set `LLVM_COV` and `LLVM_PROFDATA` env vars pointing to Homebrew's llvm binaries.

## Domain Model

Task management system (mirrors the Android test rig pattern):
- **model**: Task, Priority enum, TaskError
- **repository**: TaskRepository trait + InMemoryTaskRepository
- **usecase**: add_task, toggle_task, remove_task, get_tasks, get_stats
- **validation**: validate_title, validate_tag, sanitize_description
- **formatter**: format_task_line, format_task_list, format_stats

Intentional coverage gaps in: remove_task, toggle_task, get_tasks_by_priority, TaskError Display, format_priority_distribution.
