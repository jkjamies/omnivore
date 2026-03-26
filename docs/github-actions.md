# GitHub Actions Integration

## Basic Coverage Workflow

This workflow runs tests, generates a coverage report, and uploads it to the Omnivore dashboard.

```yaml
name: Coverage

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  coverage:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: write  # Required for PR comments
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-java@v4
        with:
          distribution: temurin
          java-version: '17'

      - uses: gradle/actions/setup-gradle@v4

      - name: Run tests and generate coverage
        run: ./gradlew test omnivoreReport

      # Upload on main branch pushes
      - name: Upload coverage (main)
        if: github.ref == 'refs/heads/main'
        run: |
          curl -s -X POST \
            "${{ vars.OMNIVORE_DASHBOARD_URL }}/api/v1/ingest/coverage" \
            -H "Content-Type: application/json" \
            -H "X-API-Key: ${{ secrets.OMNIVORE_API_KEY }}" \
            -d @build/reports/omnivore/omnivore-report.json

      # Upload on PRs with comment
      - name: Upload coverage (PR)
        if: github.event_name == 'pull_request'
        run: |
          curl -s -X POST \
            "${{ vars.OMNIVORE_DASHBOARD_URL }}/api/v1/ingest/coverage?github_repo=${{ github.repository }}&pr_number=${{ github.event.pull_request.number }}&base_branch=main" \
            -H "Content-Type: application/json" \
            -H "X-API-Key: ${{ secrets.OMNIVORE_API_KEY }}" \
            -H "X-GitHub-Token: ${{ secrets.GITHUB_TOKEN }}" \
            -d @build/reports/omnivore/omnivore-report.json

      # Save report as artifact
      - name: Upload artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: coverage-report
          path: build/reports/omnivore/
```

## Setup

### Repository Variables

Set these in **Settings > Secrets and variables > Actions > Variables**:

| Variable | Value |
|---|---|
| `OMNIVORE_DASHBOARD_URL` | Your dashboard URL (e.g., `https://omnivore.example.com`) |

### Repository Secrets

Set these in **Settings > Secrets and variables > Actions > Secrets**:

| Secret | Value |
|---|---|
| `OMNIVORE_API_KEY` | API key from the Omnivore dashboard Settings page |

> **Note:** API key authentication is optional — if no keys have been created on the dashboard, the ingest endpoint is open. Once you create your first key, all uploads require a valid `X-API-Key` header.

### Permissions

The `GITHUB_TOKEN` is automatically provided by GitHub Actions. The workflow needs `pull-requests: write` permission for PR comments.

If your repository uses restricted permissions, add this to the job:

```yaml
permissions:
  contents: read
  pull-requests: write
```

## Non-Gradle Projects

### Go

```yaml
- name: Run tests with coverage
  run: go test -coverprofile=coverage.lcov ./...

- name: Upload coverage
  run: |
    curl -s -X POST \
      "${{ vars.OMNIVORE_DASHBOARD_URL }}/api/v1/ingest/coverage?format=lcov&project_id=${{ github.event.repository.name }}&project_name=${{ github.event.repository.name }}&commit_sha=${{ github.sha }}&branch=${{ github.ref_name }}&github_repo=${{ github.repository }}&pr_number=${{ github.event.pull_request.number }}" \
      -H "X-API-Key: ${{ secrets.OMNIVORE_API_KEY }}" \
      -H "X-GitHub-Token: ${{ secrets.GITHUB_TOKEN }}" \
      -d @coverage.lcov
```

### Rust

```yaml
- name: Install cargo-llvm-cov
  run: cargo install cargo-llvm-cov

- name: Run tests with coverage
  run: cargo llvm-cov --json --output-path=llvm-cov.json

- name: Upload coverage
  run: |
    curl -s -X POST \
      "${{ vars.OMNIVORE_DASHBOARD_URL }}/api/v1/ingest/coverage?format=llvm-cov&project_id=${{ github.event.repository.name }}&project_name=${{ github.event.repository.name }}&commit_sha=${{ github.sha }}&branch=${{ github.ref_name }}&github_repo=${{ github.repository }}&pr_number=${{ github.event.pull_request.number }}" \
      -H "Content-Type: application/json" \
      -H "X-API-Key: ${{ secrets.OMNIVORE_API_KEY }}" \
      -H "X-GitHub-Token: ${{ secrets.GITHUB_TOKEN }}" \
      -d @llvm-cov.json
```

## PR Comment

When you pass `github_repo` and `pr_number` query parameters, the dashboard posts a Markdown comment on the PR:

- Coverage summary with line/branch rates
- Delta comparison against the base branch (configurable via `base_branch` param, defaults to `main`)
- Status badge: passing (>= 80%), warning (>= 50%), failing (< 50%)
- Collapsible file breakdown highlighting regressions
- Link to the full report on the dashboard

On subsequent pushes, the comment is updated in-place rather than creating duplicates.

## Using the Gradle Upload Task

Instead of `curl`, you can use the built-in Gradle task:

```yaml
- name: Upload coverage
  env:
    OMNIVORE_API_KEY: ${{ secrets.OMNIVORE_API_KEY }}
  run: ./gradlew omnivoreUpload -Pomnivore.dashboard.url=${{ vars.OMNIVORE_DASHBOARD_URL }}
```

Note: The Gradle task doesn't support PR comments directly. Use `curl` for PR comment integration.
