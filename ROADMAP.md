# Omnivore Roadmap

High-value features to make Omnivore production-ready for startups, enterprises, and dev teams.

## Access & Identity

- [ ] **GitHub OAuth login** — gate the dashboard behind auth; teams get user identity for free
- [ ] **API keys for upload auth** — token-based auth for CI uploads; `api_keys` table with hashed tokens, scoped per-project or global

## Team Workflow

- [ ] **GitHub commit status checks** — post `success`/`failure` on the commit at ingest time based on thresholds; blocks merges when coverage drops
- [ ] **Coverage gates on PR comments** — add pass/fail status to existing PR comments based on configured thresholds
- [ ] **Slack/webhook notifications** — fire a webhook when coverage drops below threshold; configurable per-project

## Visibility

- [x] **Dashboard home summary** — aggregate stats across all projects: total projects, average coverage, how many are below threshold
- [x] **Copy badge markdown button** — "copy to clipboard" button on the project settings page for README badge embedding
- [x] **Retention policy in settings** — configurable full and summary snapshot retention in the global settings UI; replaces env vars

## Developer Experience

- [x] **Project delete** — form POST with browser confirmation dialog to remove projects from the dashboard
- [x] **GitHub Action for CI** — composite action at `.github/actions/upload-coverage/` that wraps the ingest curl; auto-detects commit, branch, PR from GitHub context
- [x] **Search/filter on projects page** — client-side JS filter as the project list grows

## Data & Reporting

- [ ] **CSV/JSON trend export** — download trend data as CSV or JSON for external dashboards
- [ ] **Scheduled email/Slack digests** — weekly coverage summary sent to configured channels
