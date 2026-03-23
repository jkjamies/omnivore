# dashboard

Rust workspace providing a REST API server for coverage data storage and querying.

## Structure

```
crates/
  omnivore-core/       Library — models, parsers, SQLite storage
  omnivore-server/     Binary (omnivore-dashboard) — Axum REST API
tests/fixtures/        Test fixtures (currently empty)
```

## Build & Test

```sh
# DATABASE_URL required for sqlx compile-time query checking.
# The database must exist with tables created (run the server once, or create manually).
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo build
DATABASE_URL="sqlite:omnivore.db?mode=rwc" cargo test
```

Binary name: `omnivore-dashboard`
License: **Apache-2.0**

## Key Dependencies

| Dependency | Version | Purpose |
|---|---|---|
| axum | 0.8 | Web framework |
| sqlx | 0.8 | Async SQLite (with tokio runtime) |
| tokio | 1.x | Async runtime (full features) |
| serde / serde_json | 1.x | JSON serialization |
| chrono | 0.4 | DateTime handling |
| uuid | 1.x | Snapshot IDs (v4) |
| tower-http | 0.6 | CORS, tracing middleware |
| thiserror | 2.x | Error types |
| askama | 0.15 | HTML templating (Jinja2-style) |
| reqwest | 0.12 | HTTP client (GitHub API for PR comments) |
| tracing | 0.1 | Structured logging |

## API Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/api/v1/health` | Health check (`{"status":"ok","version":"0.1.0"}`) |
| GET | `/api/v1/projects` | List all projects |
| POST | `/api/v1/projects` | Create project (body: `{id, name, description?, github_repo?}`) |
| PATCH | `/api/v1/projects/{project_id}` | Update project settings (body: `{github_repo?}`) |
| POST | `/api/v1/ingest/coverage` | Universal ingest — omnivore JSON, lcov, llvm-cov, Go coverprofile, or Python coverage.py (auto-detects or `?format=`) |
| GET | `/api/v1/coverage/{project_id}/latest` | Latest snapshot for project |
| GET | `/api/v1/coverage/{project_id}/trend?limit=30` | Coverage trend (TrendPoints) |
| GET | `/api/v1/coverage/{project_id}/dependencies` | Dependency graph from latest snapshot |

### PR Comment Integration

The ingest endpoint supports automatic GitHub PR comments. Pass these query params alongside the upload:
- `github_repo` — repo slug (e.g., `owner/repo`)
- `pr_number` — PR number to comment on
- `base_branch` — branch to compare against (default: `main`)

GitHub token via `X-GitHub-Token` header (preferred in CI) or `GITHUB_TOKEN` env var on the server.

The comment includes: coverage summary with delta vs base branch, status badge, file breakdown (collapsible), and dashboard link. Comments are updated in-place on subsequent pushes (marker-based detection).

## Database

SQLite with embedded schema creation (no migration files). Connection pool: max 5.

**Environment variables:**
- `DATABASE_URL` — default: `sqlite:omnivore.db?mode=rwc`
- `BIND_ADDR` — default: `0.0.0.0:3000`
- `RUST_LOG` — default: `info`
- `GITHUB_TOKEN` — (optional) GitHub token for PR comments and on-demand source fetching; can also be passed per-request via `X-GitHub-Token` header
- `OMNIVORE_DASHBOARD_URL` — (optional) public URL of this server, used for "View report" links in PR comments
- `OMNIVORE_RETENTION_FULL` — (optional, default 30) newest N snapshots per project+target keep full file data
- `OMNIVORE_RETENTION_SUMMARY` — (optional, default 60) next N snapshots kept as summary-only for trend charts

**Tables:**

```sql
projects (id TEXT PK, name TEXT, description TEXT, github_repo TEXT, source_root TEXT, tags TEXT, created_at TEXT, updated_at TEXT,
          line_threshold REAL, branch_threshold REAL, line_warn_threshold REAL, branch_warn_threshold REAL)

coverage_snapshots (
    id TEXT PK, project_id TEXT FK→projects, commit_sha TEXT, branch TEXT,
    target TEXT, line_rate REAL, branch_rate REAL,
    lines_covered INT, lines_total INT, branches_covered INT, branches_total INT,
    file_count INT, created_at TEXT, files_json TEXT, dependencies_json TEXT
)
-- Index: idx_snapshots_project ON (project_id, created_at DESC)
```

## Parsers (Multi-Format Ingestion)

All parsers normalize to `(OmnivoreReport, CoverageSnapshot)` — a common model that feeds into the same storage and display pipeline.

| Parser | Module | Input Format | Use Case |
|---|---|---|---|
| Omnivore JSON | `parsers::omnivore_json` | Omnivore plugin output | Kotlin/Android/KMP projects |
| lcov | `parsers::lcov` | lcov trace files | C/C++ (gcov/lcov) |
| llvm-cov | `parsers::llvm_cov` | `llvm-cov export --format=json` | Rust (`cargo llvm-cov`), Swift/Xcode |
| Go coverprofile | `parsers::go_coverprofile` | `go test -coverprofile` output | Go projects (native format, no conversion) |
| Python coverage.py | `parsers::python_coverage` | `coverage json` output | Python projects (native format, no conversion) |

For lcov, llvm-cov, Go coverprofile, and Python coverage.py, project metadata (id, name, commit, branch) is supplied via `LcovMeta`/`LlvmCovMeta`/`GoCoverprofileMeta`/`PythonCoverageMeta` structs (mapped from query params in the API).

Format auto-detection: JSON starting with `"format":"omnivore"` → Omnivore; `"type":"llvm.coverage"` → llvm-cov; `"executed_lines"` + `"num_statements"` → Python coverage.py; lines starting with `TN:` or `SF:` → lcov; lines starting with `mode:` → Go coverprofile.

## Architecture

- `omnivore-core::parsers::omnivore_json::parse()` — deserializes report JSON into `OmnivoreReport` + `CoverageSnapshot`
- `omnivore-core::parsers::lcov::parse()` — parses lcov trace data (DA/BRDA/SF records)
- `omnivore-core::parsers::llvm_cov::parse()` — parses llvm-cov export JSON (segments → per-line coverage)
- `omnivore-core::parsers::go_coverprofile::parse()` — parses Go coverprofile (block ranges → per-line coverage)
- `omnivore-core::parsers::python_coverage::parse()` — parses Python coverage.py JSON (executed/missing lines)
- `omnivore-core::github::generate_comment()` — generates Markdown PR comment with delta comparison
- `omnivore-core::github::GitHubClient` — posts/updates PR comments via GitHub REST API
- `omnivore-core::github::source::fetch_source()` — fetches file source code on-demand from GitHub API (used by file coverage page)
- `omnivore-core::storage::db::Database` — all DB operations, auto-creates tables on `new()`
- `omnivore-server::routes::*` — Axum handlers, `State<Database>` shared state
- File-level coverage stored as JSON blob in `files_json` column
- **sqlx notes:** `chrono` feature enabled for `DateTime<Utc>` mapping; `TEXT PRIMARY KEY` columns need `as "id!: String"` non-null override in `query_as!` macros (SQLite quirk)
- **serde notes:** Model structs use `#[serde(rename_all = "camelCase")]` to match Kotlin's kotlinx-serialization output (e.g., `lineRate`, `commitSha`)

## Frontend

HTMX + Askama 0.15 templates with Chart.js for trend graphs.

**Pages:**
| Path | Template | Description |
|---|---|---|
| `/` | `projects.html` | Project list with sparklines, pinning, tags, activity log |
| `/projects/{id}` | `project_detail.html` | Stats, trend chart, hotspots, file breakdown, activity log |
| `/projects/{id}/files/{path}` | `file_coverage.html` | Source code with line-level coverage gutter marks (source fetched from GitHub API) |
| `/projects/{id}/dependencies` | `dependency_graph.html` | D3.js force-directed dependency graph |
| `/projects/{id}/settings` | `project_settings.html` | GitHub repo, source root, thresholds, tags |
| `/settings` | `settings.html` | Global thresholds, retention policy, system health link |
| `/health` | `health.html` | System health: uptime, DB size, snapshot count, last ingest |
| `/badge/{project_id}` | (SVG) | Shields.io-style coverage badge |

**Static assets:** `crates/omnivore-server/static/style.css` — responsive, dark/light theme via `prefers-color-scheme` + manual toggle (localStorage `data-theme`). Includes `source-table` styles with `border-collapse:separate`, coverage gutter marks, hit badges, and coverage row backgrounds for the file coverage view.

**Templates:** `crates/omnivore-server/templates/` — Askama HTML with `base.html` layout.

**Key patterns:**
- Helper methods on template structs (`fmt_pct`, `rate_color`, `short_sha`, `trend_json`)
- `ServeDir` serves `/static/` from the crate's `static/` directory
- Chart.js + HTMX loaded from CDN (no build step)
- Coverage thresholds: configurable per-project with global defaults (green >= threshold, yellow >= warn threshold, red < warn threshold)
- Dark/light theme toggle with localStorage persistence
- Keyboard shortcuts: `/` to focus search, `Escape` to clear/blur
- Project pinning via localStorage, sparkline trend graphs (SVG polylines)
- Project tags/labels with tag filter bar
- Ingest activity log on home page and project detail pages

**Coverage targets:** `JVM_UNIT`, `ANDROID_INSTRUMENTED`, `IOS_UNIT`, `KOTLIN_NATIVE`, `COMPOSITE`, `RUST_LLVM_COV`, `GO_COVER`, `PYTHON_COVERAGE`, `LCOV` — each parser sets the appropriate target automatically.

## Feature Tiers

All dashboard features belong to a tier. When licensing is implemented, Pro/Enterprise features will be gated at the route/handler level. See [FEATURES.md](../FEATURES.md) for the authoritative list with build status.

**Community (Free):** Multi-format ingestion, coverage trends, nested file tree, source code view, hotspots, historical deltas, gradient bars, global thresholds, home summary, search/filter, retention settings, single-snapshot export, badges, GitHub Action, project delete, dependency graph, multi-target support, Compose filtering, sparklines, pinning (localStorage), tags/labels, keyboard shortcuts, health dashboard, dark/light theme, activity log, unlimited projects.

**Pro:** GitHub PR comments on ingest, per-project threshold override, two-snapshot comparison export, server-persisted pinning (planned), GitHub OAuth (planned), GitHub commit status checks (planned), PR coverage gates (planned), Slack/Discord/webhook notifications (planned), email digests (planned), API keys (planned), diff coverage (planned), AI copy-to-clipboard prompts (planned).

**Enterprise:** SSO/SAML (planned), audit logs (planned), per-project retention (planned), inline AI suggestions (planned), PR-level AI test review (planned), Postgres HA backend (planned), priority support (planned).
