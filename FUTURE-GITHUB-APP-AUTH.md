# Future: GitHub App Authentication

Replace manual `GITHUB_TOKEN` env var and per-project `github_repo` configuration with a first-class GitHub App integration (like Codecov, SonarCloud, etc.).

## Current State

- Source code display in file coverage view requires:
  1. Setting `GITHUB_TOKEN` env var on the dashboard server
  2. Setting `github_repo` on each project (via API or at creation time)
- PR comment posting also uses `GITHUB_TOKEN` (env var or per-request header)
- This is manual and fragmented — each project needs explicit configuration

## Proposed: GitHub App Flow

### Setup
1. Register an **Omnivore GitHub App** on GitHub (org or personal)
2. App requests permissions: `contents:read` (source fetching), `pull_requests:write` (PR comments)
3. Users install the app on their repos from the GitHub Marketplace or a direct install link

### Dashboard Integration
1. **OAuth callback endpoint** — handles the GitHub App installation flow
2. **Installation token management** — store and refresh per-installation tokens in the DB
3. **Auto-discovery** — list repos where the app is installed, auto-link to Omnivore projects
4. **Source fetching** — use installation tokens instead of personal access tokens
5. **PR comments** — use installation tokens instead of `X-GitHub-Token` header

### Benefits
- No manual token management for users
- Per-repo scoping (principle of least privilege)
- Auto-discovers repos — no need to manually set `github_repo` on projects
- Tokens auto-rotate (GitHub installation tokens expire after 1 hour)
- Dashboard UI: "Connect to GitHub" button instead of API calls

### Implementation Outline
1. New DB table: `github_installations (id, installation_id, account_login, account_type, created_at)`
2. New DB table: `github_repos (installation_id, repo_full_name, project_id FK)`
3. OAuth routes: `GET /auth/github/install`, `GET /auth/github/callback`
4. Webhook endpoint: `POST /webhooks/github` (handles installation events)
5. Token service: fetches/caches installation access tokens via GitHub API
6. Replace all `GITHUB_TOKEN` usage with installation token lookup

### References
- [GitHub Apps documentation](https://docs.github.com/en/apps)
- [Creating a GitHub App](https://docs.github.com/en/apps/creating-github-apps)
- [Authenticating as a GitHub App installation](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-as-a-github-app-installation)
