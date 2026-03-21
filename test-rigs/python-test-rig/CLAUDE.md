# python-test-rig

Python project for testing Omnivore dashboard ingestion of coverage.py JSON format coverage data.

## Structure

```
taskmanager/         Domain model, repository, use cases, validation, formatting
tests/               pytest test suite
pyproject.toml       Project config with coverage.py settings
```

## Build & Test

```sh
python3 -m pytest tests/
```

## Coverage

Uses Python's native `coverage.py` tool — no conversion needed. The dashboard ingests coverage.py JSON directly via `?format=python`.

```sh
# Run tests with coverage and generate JSON
python3 -m coverage run -m pytest tests/
python3 -m coverage json

# Upload to dashboard
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=python&project_id=python-test-rig&project_name=Python+Test+Rig" \
  --data-binary @coverage.json
```

Requires: `pip install pytest coverage`

## Domain Model

Task management system (mirrors the Android/Rust/Go test rig pattern):
- **model**: Task dataclass, Priority enum, TaskError/ValidationError/NotFoundError
- **repository**: TaskRepository ABC + InMemoryTaskRepository (thread-safe with Lock)
- **usecase**: add_task, toggle_task, remove_task, get_tasks, get_tasks_by_priority, get_stats
- **validation**: validate_title, validate_tag, sanitize_description
- **formatter**: format_task_line, format_task_list, format_stats, format_priority_distribution

Intentional coverage gaps in: is_overdue, is_actionable, matches_search (description/tags), toggle_task, remove_task, update, find_by_tag, get_by_priority, format_priority_distribution.
