# Feature Tiers

Tracks which features exist in the project and which tier they belong to. Features marked **Built** are implemented; **Planned** are not yet built.

## Community (Free)

| Feature | Status |
|---|---|
| Multi-format coverage ingestion (Omnivore, lcov, llvm-cov, Go, Python, JaCoCo/Kover XML) | Built |
| Provenance tracking — `source` (tool) decoupled from `target` (environment); trends/retention keyed per `(target, source)` | Built |
| Coverage trends with Chart.js graphs | Built |
| Nested file tree with directory-level aggregates | Built |
| File-level coverage with source code view (via GitHub API) | Built |
| Uncovered code hotspots (sortable columns) | Built |
| Historical deltas (line + branch, project + file level) | Built |
| Gradient coverage bars (red → yellow → green at thresholds) | Built |
| Configurable thresholds — global defaults | Built |
| Dashboard home summary (total projects, avg coverage, pass/warn/fail counts) | Built |
| Search/filter on projects page | Built |
| Data retention — default limits (30 full / 60 summary per project+target+source series) | Built |
| Coverage badges for READMEs — Markdown, HTML, URL snippets (`/badge/{project_id}`) | Built |
| GitHub Action for CI upload (`.github/actions/upload-coverage/`) | Built |
| Project delete with confirmation | Built |
| Multi-target support (unit + instrumented shown separately + composite) | Built |
| Compose-aware filtering (auto-detected, zero-cost on non-Compose) | Built |
| Coverage sparklines on projects page | Built |
| Project favoriting / pinning (per-browser via localStorage) | Built |
| Project tags / labels | Built |
| Keyboard shortcuts (/ to search, Escape to clear) | Built |
| System health dashboard (uptime, DB size, snapshot count, last ingest) | Built |
| Dark/light theme toggle | Built |
| Ingest history / activity log (home + project pages) | Built |
| Unlimited projects | Built |
| GitHub OAuth login | Built |
| Project permissions from GitHub repo roles | Built |
| Per-user source fetching (no shared server token) | Built |
| Embeddable SVG trend charts (`/embed/{project_id}/trend`) | Built |
| Coverage ratchet (auto-advancing floor per project) | Built |

## Pro

| Feature | Status |
|---|---|
| GitHub PR comments on ingest (coverage summary, delta, file breakdown) | Built |
| Configurable thresholds — per-project override | Built |
| Export reports — single snapshot (Markdown/JSON) | Built |
| Export reports — two-snapshot comparison | Built |
| Dependency graph visualization (D3.js, Gradle projects) | Built |
| API keys + token-based upload auth | Built |
| Admin role separation (org-based or repo-based) | Built |
| Configurable retention limits | Planned |
| Project favoriting / pinning (server-persisted per-user, tentative) | Planned |
| PR coverage gates (block merges when coverage drops, via GitHub Action) | Planned |
| Slack/Discord/webhook notifications | Planned |
| Email digests (weekly/monthly summaries) | Planned |
| Diff coverage (coverage for only changed lines in a PR) | Planned |
| AI-powered test suggestions (copy-to-clipboard prompts; customer's own API key) | Planned |

## Enterprise

| Feature | Status |
|---|---|
| SSO / SAML authentication | Planned |
| Audit logs (settings changes, uploads, timestamps) | Planned |
| Per-project retention policies | Planned |
| Inline AI suggestions (dashboard calls AI API, renders inline) | Planned |
| PR-level AI test review (AI suggestions in GitHub PR comments) | Planned |
| Multi-instance / HA deployment support (Postgres backend) | Planned |
| Priority support + SLA | Planned |
