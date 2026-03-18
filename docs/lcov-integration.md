# lcov Integration

Upload coverage data from any tool that produces lcov trace files.

## Supported Tools

| Language | Tool | Command |
|---|---|---|
| Go | `go test` | `go test -coverprofile=coverage.lcov ./...` |
| C/C++ | gcov + lcov | `lcov --capture --directory . --output-file coverage.lcov` |
| Python | coverage.py | `coverage lcov -o coverage.lcov` |
| JavaScript | c8 / nyc | `c8 --reporter=lcov npm test` |

## Upload

```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=lcov&\
project_id=my-app&\
project_name=My+App&\
commit_sha=$(git rev-parse HEAD)&\
branch=$(git branch --show-current)" \
  -d @coverage.lcov
```

### Required Parameters

| Parameter | Description |
|---|---|
| `format` | Must be `lcov` |
| `project_id` | Unique project identifier (used for grouping snapshots) |

### Optional Parameters

| Parameter | Default | Description |
|---|---|---|
| `project_name` | `"lcov import"` | Display name on the dashboard |
| `commit_sha` | — | Git commit SHA |
| `branch` | — | Git branch name |
| `github_repo` | — | GitHub repo slug for PR comments (e.g., `owner/repo`) |
| `pr_number` | — | PR number to comment on |
| `base_branch` | `main` | Branch to compare against for delta |

## lcov Format Reference

The parser supports these lcov records:

| Record | Description | Example |
|---|---|---|
| `TN:` | Test name (ignored) | `TN:unit-tests` |
| `SF:` | Source file path | `SF:src/main.go` |
| `DA:` | Line execution count | `DA:10,5` (line 10, hit 5 times) |
| `BRDA:` | Branch data | `BRDA:15,0,0,1` (line 15, branch taken) |
| `BRF:` | Branches found | `BRF:4` |
| `BRH:` | Branches hit | `BRH:3` |
| `LF:` | Lines found | `LF:20` |
| `LH:` | Lines hit | `LH:18` |
| `end_of_record` | End of file block | |

`FN`, `FNDA`, `FNF`, `FNH` (function records) are parsed but not used — coverage is derived from `DA` records.

## Go Example

```sh
# Generate coverage
go test -coverprofile=coverage.out ./...

# Convert to lcov (if needed — go test -coverprofile already produces lcov-compatible output)
# For proper lcov format, use a converter like gocover-cobertura or similar

# Upload
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=lcov&project_id=my-go-service&project_name=My+Go+Service" \
  -d @coverage.out
```

## C/C++ Example

```sh
# Compile with coverage
gcc -fprofile-arcs -ftest-coverage -o myapp myapp.c

# Run tests
./myapp

# Generate lcov
lcov --capture --directory . --output-file coverage.lcov
lcov --remove coverage.lcov '/usr/*' --output-file coverage.lcov

# Upload
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=lcov&project_id=my-c-app&project_name=My+C+App" \
  -d @coverage.lcov
```

## GitHub Actions

See [GitHub Actions Integration](github-actions.md) for CI workflow examples.
