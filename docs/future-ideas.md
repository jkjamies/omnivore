# Future Ideas

Enhancements and features under consideration. Items marked with complexity estimates.

## Coverage Quality

### Diff Coverage (Medium)
Show coverage for only the lines changed in a PR. Requires resolving git diff to file/line ranges and intersecting with coverage data. High impact for code review — answers "are my new lines tested?"

### Branch Comparison (Medium)
Compare coverage across branches (e.g., main vs feature-branch), pick-two-snapshots comparison UI, line-level diff ("these specific lines became covered/uncovered"). Extends the existing historical deltas feature.

### Merge Coverage (Medium)
Unified view combining coverage from multiple test targets (unit + instrumented). Line-level union so you see what's covered by *any* test type. Useful for the composite card.

### Test-to-Code Mapping (Large)
Track which test method covers which lines. Enables "which tests should I run for this change?" Plugin-level changes to associate test identity with probe data.

## CI/CD Integration

### GitHub Check Runs (Small-Medium)
Post pass/fail status checks via GitHub Checks API on ingest. Pairs with configurable thresholds to enable branch protection enforcement.

### Ingest-Time Threshold Enforcement (Small)
Check thresholds on ingest and return pass/fail status in the API response. Foundation for GitHub Check Runs and PR coverage gates.

### Webhook Notifications (Medium)
Configurable per-project Slack/Discord/email alerts when coverage drops below threshold or changes by more than X%.

## Dashboard UX

### Multi-Branch Support (Medium-Large)
Compare coverage across branches. Schema changes for branch-aware queries, UI for branch picker. Important for teams with long-lived feature branches.

### Team/Org Grouping (Small)
Organize projects into teams or groups. Simple tagging or folder structure on the projects page. Low value until there are many projects.

### Color-Coded Tags (Small — Free)
Allow users to assign a color to each tag via the project settings UI. Requires changing tags from comma-separated text to a structured format (individual tag management with add/remove buttons, color picker per tag). Tag pills on the home page render in the user-chosen color.

### Coverage Annotations in GitHub Files (Small-Medium — Pro)
Use GitHub's Checks API annotations to mark uncovered lines directly in the PR's "Files changed" tab. Developers see coverage without leaving GitHub. Builds on top of GitHub commit status checks.

### Custom Dashboard Widgets (Medium — Pro)
Let teams configure which stats/charts appear on the home page. Drag-and-drop widget layout. Some teams care about branch coverage, others about trend direction.

### Coverage Trend Alerts (Small — Pro)
Trigger when coverage trend crosses a threshold — not just on a single ingest, but when the 7-day moving average drops. Smarter than per-ingest notifications, fewer false alarms from one bad commit.

### Coverage Trend Embeds (Small — Free)
`/embed/{project_id}/trend` — an embeddable iframe-friendly trend chart for wikis, Notion, internal docs. SVG or lightweight HTML, no auth required.

### Snapshot Annotations / Notes (Small — Pro)
Attach notes to specific snapshots: "deployed v2.3", "refactored auth module", "intentional coverage drop — removed deprecated code". Context for why coverage changed.

### Bulk Project Import (Small — Free)
Scan a GitHub org or Gradle multi-module project and auto-create projects for each module/repo. Reduces setup friction for large codebases.

### API Rate Limiting (Small — Pro)
Rate limit the ingest endpoint per API key. Prevents runaway CI from flooding the instance. Simple token bucket in memory, configurable per key.

### Server-Persisted Pinning (Small — Pro)
Extend localStorage-based pinning to server-persisted per-user favorites. Requires authentication.

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

### Per-Project Retention (Small — Enterprise)
Per-project configurable limits. Add `retention_full` and `retention_summary` columns to `projects` table, fall back to env var defaults. Important for enterprise/compliance.

### Export Report Enhancements (Medium)
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

### Copy-to-Clipboard Prompts (Small — Pro)
Generate context-aware prompts users can copy into their AI tool (Claude, ChatGPT, etc.). No API key required. Surfaces:
- **Hotspots**: prompt with file path, uncovered line ranges, and coverage context — "write unit tests for these uncovered lines"
- **File coverage view**: button that builds a prompt from specific uncovered lines asking for test suggestions
- **Delta drops**: when coverage decreased, prompt asking what tests would restore coverage in affected files

### Inline AI Suggestions (Medium — Enterprise)
User-configurable API key (Claude, OpenAI, etc.). Dashboard calls the API and renders suggestions inline. Collapsible "AI Suggestions" panel on hotspots and file coverage views. Same surfaces as copy-to-clipboard but automatic.

### PR-Level AI Test Review (Medium-Large — Enterprise)
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
- Data retention — fixed defaults (30 full / 60 summary)
- View all data in dashboard (no export)

**Pro ($X/year per instance — 30-day free trial):**
- Everything in Community
- Export reports — single snapshot (Markdown/JSON)
- Export reports — two-snapshot comparison
- Configurable retention limits
- Dependency graph visualization
- GitHub OAuth login (admin vs viewer roles)
- GitHub PR comments on ingest (coverage summary, delta, file breakdown)
- Configurable thresholds — per-project override
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
