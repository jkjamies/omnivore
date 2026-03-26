# GitHub OAuth & Role-Based Access

## Overview

GitHub OAuth App provides authentication and role-based access control for the Omnivore dashboard. Users sign in with their GitHub account. Project-level permissions are derived from the user's GitHub repo permissions — no separate role management needed.

A future migration to GitHub App is straightforward if we need webhooks, bot identity, or fine-grained installation tokens. The OAuth login flow is nearly identical; the session/permission/role logic stays unchanged.

## Backwards Compatibility

If `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` are not configured, the dashboard remains fully open (no login required). All existing behavior is preserved.

## Authentication Flow

1. User clicks "Sign in with GitHub" on the dashboard
2. Redirected to GitHub OAuth authorization page (`https://github.com/login/oauth/authorize`)
3. GitHub redirects back to `/auth/callback` with an authorization code
4. Dashboard exchanges the code for an access token via `POST https://github.com/login/oauth/access_token`
5. Dashboard fetches user profile via `GET https://api.github.com/user`
6. Creates a server-side session in SQLite, sets `HttpOnly` / `SameSite=Lax` / `Secure` session cookie
7. Subsequent requests use the session; GitHub token is used for API calls on behalf of the user

### OAuth Scopes

- `read:user` — user profile (username, avatar)
- `read:org` — org membership check (only if `OMNIVORE_GITHUB_ORG` is set)
- `repo` — repo permission checks and source code fetching (covers private repos)

## Session Storage

Server-side sessions stored in a `sessions` SQLite table:

```sql
sessions (
    id TEXT PRIMARY KEY,          -- random session token (cookie value)
    github_username TEXT NOT NULL,
    github_token TEXT NOT NULL,   -- user's OAuth access token
    avatar_url TEXT,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL       -- default: 7 days from creation
)
```

- Session ID is a cryptographically random token (not a JWT)
- Cookie: `HttpOnly`, `SameSite=Lax`, `Secure` (in production)
- Expired sessions pruned on each login or periodically

## Project-Level Permissions

Permissions are derived from the user's access to the project's linked `github_repo`. Checked via `GET /repos/{owner}/{repo}/collaborators/{username}/permission`.

| GitHub Repo Permission | View Project & Trends | Settings / Keys / Delete | Export | File Source Code |
|---|---|---|---|---|
| `admin` / `maintain` | Yes | Yes | Yes | Yes |
| `write` / `read` | Yes | No | Yes | Yes |
| `none` / no access | Yes | No | No | No |

### Key Design Decisions

- **Coverage data is always visible.** Even users with no repo access can see coverage stats, trends, and hotspots. Coverage data isn't sensitive — it's the settings/actions and source code that need protection.
- **Source code access is naturally gated.** The user's own OAuth token is used to fetch source from GitHub. If they don't have repo access, GitHub returns 404 — no extra logic needed.
- **No `github_repo` linked = public project.** Projects without a linked GitHub repo are visible to all authenticated users. Only dashboard admins can manage their settings.

### Permission Caching

Repo permission checks are cached in SQLite to avoid excessive GitHub API calls:

```sql
permission_cache (
    user_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    permission TEXT NOT NULL,      -- admin, maintain, write, read, none
    expires_at TEXT NOT NULL,       -- 5 minute TTL
    PRIMARY KEY (user_id, repo)
)
```

On each permission check: if cache exists and not expired, use it. Otherwise fetch from GitHub API and upsert. Expired rows pruned periodically.

## Dashboard Admin Access

Dashboard-level admin controls (global settings, global API keys) are determined automatically — no configuration required.

**Two-tier resolution:**

1. **If `OMNIVORE_GITHUB_ORG` is set:** GitHub org owners = dashboard admin. Org members = viewer.
2. **If no org is set:** Any user who has `admin` permission on at least one linked GitHub repo in the dashboard = dashboard admin. Everyone else = viewer.

This covers all team sizes with zero manual role management:

| Scenario | How admin is determined |
|---|---|
| Solo dev | Admin on their own repos → dashboard admin |
| Small startup (no org) | Lead has admin on repos → dashboard admin; devs are viewers |
| Enterprise (with org) | Org owners → dashboard admin; members → viewer |

**What admins can do:**
- Manage global settings (thresholds, retention)
- Create and delete global API keys
- Access all project settings (regardless of repo permission)

**What viewers can do:**
- View all projects, trends, hotspots
- Access project settings/exports/source only where their repo permissions allow

### Free vs Pro Behavior

- **Free without OAuth configured** = fully open (today's behavior)
- **Free with OAuth configured** = login required, everyone is admin (no role separation)
- **Pro with OAuth configured** = login required, roles enforced (admin/viewer distinction active)

## Feature Tier Placement

| Feature | Tier | Notes |
|---|---|---|
| GitHub OAuth login | Free | Basic security shouldn't be paywalled |
| Project permissions from repo roles | Free | Access control is fundamental |
| Per-user source fetching (no shared token) | Free | Natural consequence of OAuth, not a separate feature |
| Admin role separation | Pro | Solo/small teams don't need it — everyone's admin |
| Org-based admin resolution | Pro | Only matters for teams with an org |
| Server-persisted pinning (per-user) | Pro (tentative) | Depends on scope; localStorage pinning remains free |
| Audit logs (who did what) | Enterprise (tentative) | Depends on scope |

## API Endpoint Access

| Endpoint | Auth |
|---|---|
| `POST /api/v1/ingest/coverage` | API key (`X-API-Key` header) — unchanged |
| `GET /api/v1/coverage/*`, `GET /api/v1/projects` | Open (read-only data, used by CI tools and badges) |
| Web UI pages | OAuth session (when configured) |

The ingest and query APIs are not gated by OAuth. API keys handle ingest auth. Query endpoints stay open for CI integrations and badge rendering.

## Per-User Benefits

Once users have individual sessions:

- **Source fetching uses the user's token** instead of a shared server `GITHUB_TOKEN` — no god token
- **Server-persisted pinning** can be tied to the user
- **Audit logs** can attribute actions to users
- **Configurable retention** can be admin-only

## Routes to Add

| Method | Path | Description |
|---|---|---|
| `GET` | `/auth/login` | Redirect to GitHub OAuth authorization |
| `GET` | `/auth/callback` | Handle OAuth callback, create session |
| `POST` | `/auth/logout` | Destroy session, clear cookie |
| `GET` | `/auth/me` | Current user info (for UI display) |

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `GITHUB_CLIENT_ID` | No | GitHub OAuth App client ID (enables OAuth login) |
| `GITHUB_CLIENT_SECRET` | No | GitHub OAuth App client secret |
| `OMNIVORE_GITHUB_ORG` | No | GitHub organization slug; if set, org owners = admin, members = viewer (Pro) |
| `GITHUB_TOKEN` | No | Fallback token for PR comments from CI (no logged-in user) |

## Future: GitHub App Migration

If we later need webhooks (push-triggered coverage checks), bot identity (PR comments from "Omnivore" instead of a user), or fine-grained installation tokens, we can migrate to a GitHub App. The migration is small:

1. Register a GitHub App on github.com (setup, not code)
2. Update token exchange endpoint + add refresh token logic (~20 lines)
3. Add webhook endpoint (new route, only if we want push events)
4. Add installation token caching (only if we want bot actions)

Session logic, permission checks, roles, and UI remain unchanged.
