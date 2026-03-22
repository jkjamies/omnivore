# Future Ideas

Enhancements and features under consideration. Items marked with complexity estimates.

## Coverage Quality

### Diff Coverage (Medium)
Show coverage for only the lines changed in a PR. Requires resolving git diff to file/line ranges and intersecting with coverage data. High impact for code review — answers "are my new lines tested?"

### Historical Comparison / Deltas (Done — basic; future: branch comparison)
Shows delta between latest and previous snapshot on project list, detail page, and file tree. Future: compare coverage across branches (e.g., main vs feature-branch), pick-two-snapshots comparison UI, line-level diff ("these specific lines became covered/uncovered").

### Uncovered Code Hotspots (Done)
Ranks files by number of uncovered lines on the project detail page. Sortable columns on the file tree (click Lines/Branches headers to sort ascending/descending, click again to restore tree view).

### Merge Coverage (Medium)
Unified view combining coverage from multiple test targets (unit + instrumented). Line-level union so you see what's covered by *any* test type. Useful for the composite card.

### Test-to-Code Mapping (Large)
Track which test method covers which lines. Enables "which tests should I run for this change?" Plugin-level changes to associate test identity with probe data.

## CI/CD Integration

### GitHub Check Runs (Small-Medium)
Post pass/fail status checks via GitHub Checks API on ingest. Pairs with configurable thresholds to enable branch protection enforcement.

### Coverage Thresholds / Gates (Small-Medium)
Configurable per-project pass/fail thresholds. Check on ingest, return pass/fail status. Could block PRs when combined with GitHub Check Runs.

### Badge Endpoint (Done)
`/badge/{project_id}` — shields.io-style SVG badge for READMEs. Supports `?metric=branch` and `?target=` query params.

### Webhook Notifications (Medium)
Configurable per-project Slack/Discord/email alerts when coverage drops below threshold or changes by more than X%.

## Dashboard UX

### Search/Filter in File Tree (Done)
Client-side text filter on file breakdown tree. Matches against full file paths, shows ancestor directories of matches.

### Multi-Branch Support (Medium-Large)
Compare coverage across branches. Schema changes for branch-aware queries, UI for branch picker. Important for teams with long-lived feature branches.

### Team/Org Grouping (Small)
Organize projects into teams or groups. Simple tagging or folder structure on the projects page. Low value until there are many projects.

### Gradient Coverage Bars (Done)
Coverage bars use red→yellow→green gradient with color transitions at threshold points (50%, 80%). Visually communicates proximity to thresholds.

### Nested File Tree (Done)
Directory-grouped file breakdown with collapsible folders, path collapsing for single-child directories, and aggregate coverage per directory.

## Data Management

### Retention Policy (Done — basic; future: per-project config)
Server-wide retention via env vars, pruned on each ingest:
- `OMNIVORE_RETENTION_FULL` (default 30) — newest N snapshots per project+target keep full file data
- `OMNIVORE_RETENTION_SUMMARY` (default 60) — next N snapshots kept as summary-only (`files_json` nulled) for trend charts
- Beyond that: deleted automatically

**Future**: per-project configurable limits (important for enterprise/compliance). Add `retention_full` and `retention_summary` columns to `projects` table, fall back to env var defaults.

### Export Reports (Done — basic; future: richer detail)
Project-level export at `/projects/{id}/export` with two snapshot pickers (compare any two points in time) and Markdown/JSON format selector. Report includes: overview stats, file distribution by threshold, per-target breakdown with deltas and status. Standalone mode (no comparison) also supported.

**Future enhancements:**
- Richer statistics: coverage trends over selected time period, package/directory-level aggregates, complexity-weighted coverage
- Comparative file change summaries: counts of improved/regressed/new files between snapshots
- Additional formats: PDF, HTML standalone
- All-projects summary report (once multiple projects exist)
- Scheduled/automated report generation (e.g., weekly summary emails)

## Platform

### Authentication (Medium-Large)
API keys per project, or GitHub OAuth for the dashboard. Session management, middleware, UI.

### Multi-Tenancy (Large)
Orgs/teams with access control. Depends on authentication being in place first.

### GitHub App Integration (Large)
Replace manual `GITHUB_TOKEN` env var and per-project `github_repo` with a first-class GitHub App (like Codecov, SonarCloud). Benefits: no manual token management, per-repo scoping, auto-discovery of repos, auto-rotating tokens. Requires: OAuth callback endpoints, installation token management, webhook endpoint for installation events, DB tables for installations and repo mappings.

## AI-Powered Suggestions

### Copy-to-Clipboard Prompts (Small)
Generate context-aware prompts users can copy into their AI tool (Claude, ChatGPT, etc.). No API key required. Surfaces:
- **Hotspots**: prompt with file path, uncovered line ranges, and coverage context — "write unit tests for these uncovered lines"
- **File coverage view**: button that builds a prompt from specific uncovered lines asking for test suggestions
- **Delta drops**: when coverage decreased, prompt asking what tests would restore coverage in affected files

### Inline AI Suggestions (Medium)
User-configurable API key (Claude, OpenAI, etc.). Dashboard calls the API and renders suggestions inline. Collapsible "AI Suggestions" panel on hotspots and file coverage views. Same surfaces as copy-to-clipboard but automatic.

### PR-Level AI Test Review (Medium-Large)
On ingest with PR context, generate AI-powered test suggestions as part of the GitHub PR comment. Identifies uncovered new/changed lines and suggests specific tests to write. Depends on Diff Coverage being implemented first.

## Plugin/Agent

### iOS / Kotlin Native Support (Very Large)
Instrument Kotlin/Native targets. Entirely new instrumentation approach beyond JVM bytecode.

### Multi-Platform Dependency Graphs (Medium)
Extend dependency graph extraction beyond Gradle to other ecosystems. Current Gradle/JVM support is complete (plugin extracts from `project.configurations`, dashboard visualizes with D3.js). Future platforms:
- **Rust**: `cargo metadata` → parse `resolve.nodes` (low complexity)
- **Go**: `go mod graph` → parse space-separated pairs (low complexity)
- **Swift/SPM**: `swift package show-dependencies --format=json` (low complexity)
- **JS/npm**: `npm ls --json` (medium — massive trees, need depth cap)
- **Python/pip**: `pipdeptree --json` (low complexity)

Recommended approach: start with platform-specific CLI tools/scripts, later consolidate into a universal `omnivore` CLI binary. The existing `DependencyGraph` data model is already platform-agnostic — just needs different producers.
