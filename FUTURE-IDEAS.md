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

### Coverage Thresholds / Gates (Done — basic; future: ingest-time enforcement)
Per-project configurable line and branch coverage thresholds with global defaults. Global settings page (`/settings`) and per-project override via modal on project detail page. Thresholds drive status badges, coverage bar colors, and badge endpoint colors. Projects inherit global defaults unless overridden (NULL = inherit). Future: check thresholds on ingest and return pass/fail status, block PRs when combined with GitHub Check Runs.

### Badge Endpoint (Done)
`/badge/{project_id}` — shields.io-style SVG badge for READMEs. Supports `?metric=branch` and `?target=` query params.

### GitHub Action for Upload (Small)
Reusable composite GitHub Action (`jkjamies/omnivore-upload@v1`) that wraps the `curl` ingest call. Inputs: `server`, `project_id`, `format`, `file`. Automatically pulls commit SHA, branch, and PR number from GitHub context. Requires the repo to be public (or published to Marketplace) for external consumption. Build once the repo goes public.

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

## Scalability

### Relational File/Line Storage (Medium)
Replace `files_json` TEXT blob with normalized tables (`coverage_files`, `coverage_lines`). Current blob approach works but at scale (1-2M LOC projects, 20-60 merges/day) deserialization and DB size become concerns. Normalized tables enable: per-file queries without parsing the full blob, SQL-level aggregation ("which files dropped?"), proper indexing, and smaller per-query reads. Tradeoff: more complex inserts (batch row inserts vs single blob write). Consider when blob sizes regularly exceed ~20MB or ingest latency becomes noticeable.

### Database Backend Options (Medium)
SQLite is ideal for the self-hosted model — no external dependencies, zero config, single file backup. At very high ingest volume (hundreds of concurrent CI uploads), the single-writer lock could queue writes. Options:
- **SQLite (default)** — perfect for self-hosted single-instance deployments, which is the primary model
- **PostgreSQL (optional)** — for customers who need HA, read replicas, or already have Postgres infrastructure. sqlx already supports both; migration is mostly connection/query layer changes. Could support both via env var selection.

### Distribution & Packaging (Small-Medium)
Omnivore is self-hosted — each customer runs their own instance on their own infrastructure. Distribution options:
- **Docker image** — single `docker run` with a volume for the SQLite file. Easiest for most teams.
- **Static binary** — download and run. No runtime dependencies.
- **Helm chart** — for Kubernetes-native teams
- **One-line install script** — `curl | sh` style for quick setup

The self-hosted model means:
- Zero infrastructure cost for Omnivore (customers provide their own compute)
- SQLite is a feature, not a limitation — no DB server to manage
- Each instance is isolated — no multi-tenancy concerns
- Monetization via licensing, not hosting costs

**Features that would add cost to the customer:**
- Email digests — requires an SMTP server or email service
- AI-powered suggestions — per-call API costs (Claude, OpenAI) using the customer's own API key
- GitHub App — free, but requires a publicly reachable webhook endpoint

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

## New Feature Ideas (Unsorted by Tier)

### Dark/Light Theme Toggle (Small — Free)
Currently relies on `prefers-color-scheme`. Add an explicit toggle in the header so users can override OS preference. Stored in localStorage, no server change.

### Project Favoriting / Pinning (Small — Free)
Pin frequently used projects to the top of the dashboard. LocalStorage-based for free tier, server-persisted per-user with auth (Pro).

### Coverage Sparklines on Projects Page (Small — Free)
Tiny inline trend graph (last 10-20 points) next to each project card on the home page. Gives at-a-glance trend without clicking into a project. Pure SVG, no Chart.js needed.

### Keyboard Navigation (Small — Free)
Arrow keys to navigate project list, `/` to focus the search filter, `Esc` to clear. Power-user UX at zero cost.

### Ingest History / Activity Log (Small — Free)
Simple table showing recent ingests: timestamp, project, target, commit SHA, coverage delta. Answers "when was data last uploaded?" without clicking into each project. Useful for debugging CI pipelines.

### Coverage Annotations in GitHub Files (Small-Medium — Pro)
Use GitHub's Checks API annotations to mark uncovered lines directly in the PR's "Files changed" tab. Developers see coverage without leaving GitHub. Builds on top of GitHub commit status checks.

### Custom Dashboard Widgets (Medium — Pro)
Let teams configure which stats/charts appear on the home page. Drag-and-drop widget layout. Some teams care about branch coverage, others about trend direction.

### Coverage Trend Alerts (Small — Pro)
Trigger when coverage trend crosses a threshold — not just on a single ingest, but when the 7-day moving average drops. Smarter than per-ingest notifications, fewer false alarms from one bad commit.

### Project Tags / Labels (Small — Free)
Tag projects with labels (e.g., "backend", "mobile", "critical"). Filter the projects page by tag. Simple metadata column, no auth needed.

### API Rate Limiting (Small — Pro)
Rate limit the ingest endpoint per API key. Prevents runaway CI from flooding the instance. Simple token bucket in memory, configurable per key.

### Coverage Trend Embeds (Small — Free)
`/embed/{project_id}/trend` — an embeddable iframe-friendly trend chart for wikis, Notion, internal docs. SVG or lightweight HTML, no auth required.

### Snapshot Annotations / Notes (Small — Pro)
Attach notes to specific snapshots: "deployed v2.3", "refactored auth module", "intentional coverage drop — removed deprecated code". Context for why coverage changed.

### Multi-Format Badge Endpoint (Small — Free)
Extend badge to support additional formats beyond shields.io SVG — JSON endpoint for custom badge rendering, HTML badge for wikis/docs.

### Bulk Project Import (Small — Free)
Scan a GitHub org or Gradle multi-module project and auto-create projects for each module/repo. Reduces setup friction for large codebases.

### Health Check Dashboard (Small — Free)
Expand `/api/v1/health` to include: database size, snapshot count, last ingest time, uptime. Useful for monitoring the Omnivore instance itself.

## Monetization

### Freemium Open-Core Model

Self-hosted, free core with paid tiers for advanced features. Free trial period for paid tiers so teams can evaluate before committing.

**Community (Free forever):**
- Unlimited projects
- All coverage format ingestion (Omnivore, lcov, llvm-cov, Go, Python)
- Coverage trends, file trees, hotspots
- Configurable thresholds (global defaults)
- Coverage badges for READMEs
- GitHub Action for CI upload
- Data retention settings
- Export reports (single snapshot)

**Pro ($X/year per instance — 30-day free trial):**
- Everything in Community
- GitHub OAuth login (admin vs viewer roles)
- GitHub PR comments on ingest (coverage summary, delta, file breakdown)
- Configurable thresholds — per-project override
- Export reports — two-snapshot comparison
- GitHub commit status checks (pass/fail on PRs)
- PR coverage gates (block merges when coverage drops)
- Slack/Discord/webhook notifications
- Email digests (weekly/monthly summaries)
- API keys + token-based upload auth
- Diff coverage (coverage for only changed lines in a PR)
- AI-powered test suggestions (copy-to-clipboard prompts from hotspots + file views; uses customer's own AI API key, cost is theirs)

**Enterprise ($X/year per instance — 30-day free trial):**
- Everything in Pro
- SSO / SAML authentication
- Audit logs (who changed settings, who uploaded, when)
- Per-project retention policies
- Inline AI suggestions (dashboard calls AI API directly, renders inline)
- PR-level AI test review (AI suggestions in GitHub PR comments)
- Multi-instance / HA deployment support (Postgres backend)
- Priority support + SLA

### Licensing Implementation (Small-Medium)
- License key file checked at startup, controls which tier is active
- Key encodes: tier, expiration date, instance ID
- Expired Pro/Enterprise gracefully downgrades to Community (no data loss, features just hide)
- Free trial: generate a 30-day Pro/Enterprise key from the Omnivore website
- No phone-home required (enterprise customers dislike it); offline validation via signed keys
- Dashboard shows current tier + expiration in the settings page

### Competitive Positioning
- **vs SonarQube**: $15k+/year Enterprise, heavy JVM stack, complex setup → Omnivore: fraction of the cost, Rust-fast, 5-minute setup
- **vs Codecov/Coveralls**: $10-29/seat/month SaaS, no self-hosted option → Omnivore: flat per-instance pricing, self-hosted, data stays on your infra
- **vs JaCoCo alone**: Free but no dashboard, no trends, no multi-format, no PR integration → Omnivore: full platform on top
