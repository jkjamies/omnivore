# Omnivore Dashboard

REST API server and web frontend for coverage data storage, visualization, and PR integration.

## Quick Start

```sh
# Build
cargo build

# Run (creates omnivore.db automatically)
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run

# Dashboard available at http://localhost:3000
```

## Configuration

All configuration is via environment variables:

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | `sqlite:omnivore.db?mode=rwc` | SQLite connection string |
| `BIND_ADDR` | `0.0.0.0:3000` | Server bind address |
| `RUST_LOG` | `info` | Log level (tracing filter) |
| `GITHUB_TOKEN` | — | GitHub token for PR comments (optional) |
| `OMNIVORE_DASHBOARD_URL` | — | Public URL for "View report" links in PR comments |
| `OMNIVORE_RETENTION_FULL` | `30` | Full snapshots to keep per project+target |
| `OMNIVORE_RETENTION_SUMMARY` | `60` | Summary-only snapshots to keep beyond full |

## API Endpoints

### Coverage Ingestion

```
POST /api/v1/ingest/coverage
```

Universal ingestion endpoint. Accepts omnivore JSON, lcov, llvm-cov, Go coverprofile, and Python coverage.py formats.

**Auto-detection**: The format is detected from the content. Override with `?format=omnivore|lcov|llvm-cov|go|python`.

**Omnivore JSON** (from Gradle plugin):
```sh
curl -X POST http://localhost:3000/api/v1/ingest/coverage \
  -H "Content-Type: application/json" \
  -d @omnivore-report.json
```

**Go coverprofile**:
```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=go&project_id=my-app&project_name=My+App&\
commit_sha=abc123&branch=main" \
  -d @coverage.out
```

**Python coverage.py**:
```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=python&project_id=my-app&project_name=My+App&\
commit_sha=abc123&branch=main" \
  -d @coverage.json
```

**lcov** (C/C++):
```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=lcov&project_id=my-app&project_name=My+App&\
commit_sha=abc123&branch=main" \
  -d @coverage.lcov
```

**llvm-cov** (Rust, Swift):
```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
format=llvm-cov&project_id=my-app&project_name=My+App&\
commit_sha=abc123&branch=main" \
  -d @llvm-cov-export.json
```

**With PR comment** (any format):
```sh
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?\
github_repo=owner/repo&pr_number=42&base_branch=main" \
  -H "Content-Type: application/json" \
  -H "X-GitHub-Token: ghp_xxxxx" \
  -d @omnivore-report.json
```

### Coverage Queries

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/v1/projects` | List all projects |
| `POST` | `/api/v1/projects` | Create a project |
| `PATCH` | `/api/v1/projects/{project_id}` | Update project settings |
| `GET` | `/api/v1/coverage/{project_id}/latest` | Latest coverage snapshot |
| `GET` | `/api/v1/coverage/{project_id}/trend?limit=30` | Coverage trend data |
| `GET` | `/api/v1/coverage/{project_id}/dependencies` | Dependency graph |
| `GET` | `/api/v1/health` | Health check |

### Web Pages

| Path | Description |
|---|---|
| `/` | Project list with sparklines, pinning, tags, activity log |
| `/projects/{id}` | Coverage stats, trend chart, hotspots, file tree, activity log |
| `/projects/{id}/files/{path}` | Source code with line-level coverage annotations |
| `/projects/{id}/settings` | GitHub repo, source root, thresholds, tags |
| `/projects/{id}/dependencies` | D3.js dependency graph visualization |
| `/settings` | Global thresholds, retention policy |
| `/health` | System health (uptime, DB size, snapshots, last ingest) |
| `/badge/{project_id}` | Shields.io-style SVG coverage badge |

## PR Comment Integration

The dashboard can post coverage summaries as comments on GitHub pull requests. Comments include:

- Coverage summary table (line rate, branch rate, file count)
- Delta comparison against the base branch
- Status badge (passing/warning/failing)
- Collapsible file breakdown showing regressions first
- Link to the full report on the dashboard

### How It Works

1. CI uploads coverage with PR metadata via query params
2. Dashboard ingests the snapshot and stores it
3. Dashboard looks up the latest snapshot on the base branch for comparison
4. Generates a Markdown comment and posts it via the GitHub API
5. On subsequent pushes, the existing comment is updated (not duplicated)

### GitHub Token

The token needs `pull_requests:write` permission. Provide it either:
- **Per-request** via `X-GitHub-Token` header (recommended for CI — use `${{ secrets.GITHUB_TOKEN }}`)
- **Server-wide** via `GITHUB_TOKEN` environment variable

## Deployment

### Docker (example)

```dockerfile
FROM rust:1.83 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/omnivore-dashboard /usr/local/bin/
ENV DATABASE_URL="sqlite:/data/omnivore.db?mode=rwc"
EXPOSE 3000
CMD ["omnivore-dashboard"]
```

### Fly.io / Railway / Render

Set these environment variables:
- `DATABASE_URL` — use a persistent volume for SQLite
- `BIND_ADDR` — typically `0.0.0.0:3000` (or `0.0.0.0:$PORT` for Railway)
- `GITHUB_TOKEN` — for PR comments
- `OMNIVORE_DASHBOARD_URL` — the public URL of the deployment

## Development

```sh
# Build (needs tables for sqlx compile-time checks)
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo build

# Test
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo test

# Run with debug logging
RUST_LOG=debug DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo run
```

If building from a clean state, create the database first by running the server once (it auto-creates tables), or use the `sqlite3` CLI to create the schema manually.

## Feature Tiers

Omnivore uses a freemium open-core model. All current dashboard features are available in the free Community tier. See [FEATURES.md](../FEATURES.md) for the full breakdown.

| Tier | Includes |
|---|---|
| **Community (Free)** | All coverage formats, trends, file tree, hotspots, badges, thresholds, export, tags, pinning, activity log, health dashboard, dark/light theme |
| **Pro** | PR comments, per-project thresholds, two-snapshot export, GitHub OAuth, status checks, coverage gates, notifications, diff coverage, AI prompts |
| **Enterprise** | SSO/SAML, audit logs, per-project retention, inline AI, PR-level AI review, Postgres HA, priority support |

## License

Apache-2.0
