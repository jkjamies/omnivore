# llvm-cov Integration

Upload coverage data from tools that produce `llvm-cov export` JSON output.

## Supported Tools

| Language | Tool | Command |
|---|---|---|
| Rust | cargo-llvm-cov | `cargo llvm-cov --json --output-path=coverage.json` |
| Swift | llvm-cov | `xcrun llvm-cov export -format=text ...` |
| C/C++ | llvm-cov | `llvm-cov export -format=text -instr-profile=... binary` |

## Upload

```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=llvm-cov&\
project_id=my-app&\
project_name=My+App&\
commit_sha=$(git rev-parse HEAD)&\
branch=$(git branch --show-current)" \
  -H "Content-Type: application/json" \
  -d @coverage.json
```

### Required Parameters

| Parameter | Description |
|---|---|
| `format` | Must be `llvm-cov` (also accepts `llvm_cov` or `llvmcov`) |
| `project_id` | Unique project identifier |

### Optional Parameters

| Parameter | Default | Description |
|---|---|---|
| `project_name` | `"llvm-cov import"` | Display name on the dashboard |
| `commit_sha` | — | Git commit SHA |
| `branch` | — | Git branch name |
| `github_repo` | — | GitHub repo slug for PR comments |
| `pr_number` | — | PR number to comment on |
| `base_branch` | `main` | Branch to compare against |

## Rust Example

### Install cargo-llvm-cov

```sh
cargo install cargo-llvm-cov
```

### Generate and Upload

```sh
# Run tests with coverage and export as JSON
cargo llvm-cov --json --output-path=coverage.json

# Upload to dashboard
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=llvm-cov&\
project_id=my-rust-app&\
project_name=My+Rust+App&\
commit_sha=$(git rev-parse HEAD)&\
branch=$(git branch --show-current)" \
  -H "Content-Type: application/json" \
  -d @coverage.json
```

### One-Liner for CI

```sh
cargo llvm-cov --json | curl -s -X POST \
  "http://localhost:3000/api/v1/ingest/coverage?format=llvm-cov&project_id=my-app" \
  -H "Content-Type: application/json" \
  -d @-
```

## Swift / Xcode Example

### Generate Coverage

```sh
# Run tests with coverage enabled
xcodebuild test \
  -scheme MyApp \
  -destination 'platform=iOS Simulator,name=iPhone 15' \
  -enableCodeCoverage YES \
  -resultBundlePath TestResults.xcresult

# Export profdata
xcrun llvm-profdata merge -sparse \
  TestResults.xcresult/*/Coverage.profdata \
  -o merged.profdata

# Export as JSON
xcrun llvm-cov export \
  -format=text \
  -instr-profile=merged.profdata \
  path/to/MyApp.app/MyApp \
  > coverage.json
```

### Upload

```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=llvm-cov&\
project_id=my-ios-app&\
project_name=My+iOS+App" \
  -H "Content-Type: application/json" \
  -d @coverage.json
```

## Expected JSON Format

The parser expects the standard `llvm-cov export --format=text` output:

```json
{
  "type": "llvm.coverage.json.export",
  "version": "2.0.1",
  "data": [
    {
      "files": [
        {
          "filename": "src/main.rs",
          "segments": [
            [1, 1, 5, true, true],
            [3, 1, 0, true, true]
          ],
          "summary": {
            "lines": { "count": 10, "covered": 8, "percent": 80.0 },
            "branches": { "count": 4, "covered": 3, "percent": 75.0 }
          }
        }
      ],
      "totals": {
        "lines": { "count": 100, "covered": 80, "percent": 80.0 },
        "branches": { "count": 20, "covered": 15, "percent": 75.0 }
      }
    }
  ]
}
```

Segments are `[line, column, count, has_count, is_region_entry]`. The parser converts segments to per-line coverage by tracking execution counts at line boundaries.

## GitHub Actions

See [GitHub Actions Integration](github-actions.md) for CI workflow examples.
