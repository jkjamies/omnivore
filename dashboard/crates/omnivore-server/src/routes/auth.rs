use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use omnivore_core::model::session::AuthUser;
use omnivore_core::storage::Database;
use serde::{Deserialize, Serialize};

const SESSION_COOKIE: &str = "omnivore_session";
const OAUTH_STATE_COOKIE: &str = "omnivore_oauth_state";

/// Scopes requested at login.
///
/// The default is deliberately read-only and does **not** include `repo`.
/// `repo` grants full read *and write* access to every private repository the
/// user can reach, and the resulting token is stored server-side for the life
/// of the session — far more authority than a coverage dashboard needs to show
/// a trend line. Operators who want the on-demand source view to work for
/// private repositories opt in explicitly:
///
/// ```text
/// OMNIVORE_GITHUB_SCOPES=read:user,read:org,repo
/// ```
const DEFAULT_OAUTH_SCOPES: &str = "read:user,read:org";

fn oauth_scopes() -> String {
    std::env::var("OMNIVORE_GITHUB_SCOPES")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_OAUTH_SCOPES.to_string())
}

/// Whether to mark auth cookies `Secure`.
///
/// Defaults to on when the dashboard is advertised over HTTPS, off otherwise —
/// a `Secure` cookie is simply never sent over plain HTTP, which would break
/// login for someone running this on `http://localhost:3000`. Override with
/// `OMNIVORE_COOKIE_SECURE=true|false`.
pub(crate) fn cookie_secure() -> bool {
    match std::env::var("OMNIVORE_COOKIE_SECURE") {
        Ok(v) if !v.trim().is_empty() => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        _ => std::env::var("OMNIVORE_DASHBOARD_URL")
            .map(|url| url.trim_start().starts_with("https://"))
            .unwrap_or(false),
    }
}

/// Constant-time string comparison for the OAuth state token.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// OAuth configuration, loaded from environment.
#[derive(Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

impl OAuthConfig {
    /// Returns None if OAuth is not configured (dashboard stays open).
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("GITHUB_CLIENT_ID").ok()?;
        let client_secret = std::env::var("GITHUB_CLIENT_SECRET").ok()?;
        if client_id.is_empty() || client_secret.is_empty() {
            return None;
        }
        Some(Self {
            client_id,
            client_secret,
        })
    }
}

/// Redirect to GitHub OAuth authorization page.
///
/// Mints a single-use `state` token, stashes it in a short-lived cookie, and
/// echoes it to GitHub. `callback` refuses any response whose `state` doesn't
/// match, which is what stops an attacker from feeding the victim's browser an
/// authorization code of the attacker's own — a login-CSRF that would leave the
/// victim silently operating the dashboard as the attacker's GitHub identity.
pub async fn login(State(config): State<OAuthConfig>, jar: CookieJar) -> (CookieJar, Redirect) {
    let state = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );

    // A session cookie: it lives only until the callback consumes it, and the
    // callback removes it either way.
    let state_cookie = Cookie::build((OAUTH_STATE_COOKIE, state.clone()))
        .path("/")
        .http_only(true)
        .secure(cookie_secure())
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    // client_id and the scope list are operator-configured; state is hex.
    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope={}&state={}",
        config.client_id,
        oauth_scopes(),
        state,
    );

    (jar.add(state_cookie), Redirect::temporary(&url))
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GitHubUser {
    login: String,
    avatar_url: Option<String>,
}

/// Handle the OAuth callback from GitHub.
pub async fn callback(
    State((db, config)): State<(Database, OAuthConfig)>,
    jar: CookieJar,
    Query(params): Query<CallbackParams>,
) -> Result<(CookieJar, Redirect), (StatusCode, String)> {
    // Verify the CSRF state before spending the code. Both halves must be
    // present and equal; a missing cookie is as much a failure as a mismatch.
    let expected_state = jar
        .get(OAUTH_STATE_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Missing OAuth state cookie — restart login from /auth/login".to_string(),
        ))?;
    let provided_state = params.state.as_deref().unwrap_or_default();
    if !constant_time_eq(&expected_state, provided_state) {
        return Err((
            StatusCode::BAD_REQUEST,
            "OAuth state mismatch — possible login CSRF; restart login".to_string(),
        ));
    }
    // Single use, whatever happens next.
    let jar = jar.remove(Cookie::build((OAUTH_STATE_COOKIE, "")).path("/").build());

    // Exchange code for access token
    let client = reqwest::Client::new();
    let token_resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "client_id": config.client_id,
            "client_secret": config.client_secret,
            "code": params.code,
        }))
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("GitHub token exchange failed: {e}")))?;

    let token_data: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid token response: {e}")))?;

    // Fetch user profile
    let user_resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token_data.access_token))
        .header("User-Agent", "omnivore-dashboard")
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("GitHub user fetch failed: {e}")))?;

    let github_user: GitHubUser = user_resp
        .json()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("Invalid user response: {e}")))?;

    // Create session
    let session = db
        .create_session(
            &github_user.login,
            &token_data.access_token,
            github_user.avatar_url.as_deref(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Session creation failed: {e}")))?;

    // Prune old sessions occasionally
    let _ = db.prune_expired_sessions().await;

    // Set session cookie
    let cookie = Cookie::build((SESSION_COOKIE, session.id))
        .path("/")
        .http_only(true)
        .secure(cookie_secure())
        .same_site(axum_extra::extract::cookie::SameSite::Lax)
        .build();

    Ok((jar.add(cookie), Redirect::to("/")))
}

/// Destroy session and clear cookie.
pub async fn logout(
    State(db): State<Database>,
    jar: CookieJar,
) -> (CookieJar, Redirect) {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        let _ = db.delete_session(cookie.value()).await;
    }

    let removal = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .http_only(true)
        .secure(cookie_secure())
        .build();

    (jar.remove(removal), Redirect::to("/"))
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    pub oauth_enabled: bool,
    pub logged_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

/// Is OAuth configured? When it is not, the dashboard runs fully open by
/// design and every guard below short-circuits.
pub fn oauth_enabled() -> bool {
    OAuthConfig::from_env().is_some()
}

/// Get auth status: whether OAuth is configured and current user info.
pub async fn me(
    State(db): State<Database>,
    jar: CookieJar,
) -> Json<AuthStatusResponse> {
    let oauth_enabled = OAuthConfig::from_env().is_some();
    let user = extract_user(&db, &jar).await;
    Json(AuthStatusResponse {
        oauth_enabled,
        logged_in: user.is_some(),
        username: user.as_ref().map(|u| u.username.clone()),
        avatar_url: user.as_ref().and_then(|u| u.avatar_url.clone()),
    })
}

/// Extract the authenticated user from the session cookie.
/// Returns None if not authenticated or session expired.
pub async fn extract_user(db: &Database, jar: &CookieJar) -> Option<AuthUser> {
    let cookie = jar.get(SESSION_COOKIE)?;
    let session = db.get_session(cookie.value()).await.ok()??;
    Some(AuthUser {
        username: session.github_username,
        github_token: session.github_token,
        avatar_url: session.avatar_url,
    })
}

/// Check the user's permission on a GitHub repo.
/// Returns the permission string: "admin", "maintain", "write", "read", or "none".
pub async fn check_repo_permission(
    db: &Database,
    user: &AuthUser,
    repo: &str,
) -> String {
    // Check cache first
    if let Ok(Some(cached)) = db.get_cached_permission(&user.username, repo).await {
        return cached;
    }

    // Fetch from GitHub
    let permission = fetch_repo_permission(&user.github_token, &user.username, repo)
        .await
        .unwrap_or_else(|| "none".to_string());

    // Cache the result
    let _ = db.cache_permission(&user.username, repo, &permission).await;

    permission
}

async fn fetch_repo_permission(token: &str, username: &str, repo: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/collaborators/{}/permission",
        repo, username
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "omnivore-dashboard")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return Some("none".to_string());
    }

    #[derive(Deserialize)]
    struct PermResp {
        permission: String,
    }

    let data: PermResp = resp.json().await.ok()?;
    Some(data.permission)
}

/// Check if the user is a *dashboard* admin — able to change global settings
/// and mint global API keys.
///
/// Resolution order:
/// 1. `OMNIVORE_ADMIN_USERS` — an explicit comma-separated allowlist of GitHub
///    usernames. Unambiguous, and the recommended setting for a self-hosted
///    instance.
/// 2. `OMNIVORE_GITHUB_ORG` — owners of that org are admins.
/// 3. Otherwise: nobody. There is deliberately no "admin on any linked repo"
///    fallback. Projects are auto-created by ingest, and anyone who can reach
///    an open ingest endpoint can create one pointing at a repo they own — so
///    that rule let an outsider promote themselves to dashboard admin by
///    uploading a report and linking their own repository.
pub async fn is_dashboard_admin(db: &Database, user: &AuthUser) -> bool {
    let _ = db;

    if let Ok(list) = std::env::var("OMNIVORE_ADMIN_USERS") {
        if !list.trim().is_empty() {
            return list
                .split(',')
                .map(str::trim)
                .any(|name| !name.is_empty() && name.eq_ignore_ascii_case(&user.username));
        }
    }

    if let Ok(org) = std::env::var("OMNIVORE_GITHUB_ORG") {
        if !org.trim().is_empty() {
            return check_org_owner(&user.github_token, &user.username, org.trim()).await;
        }
    }

    tracing::warn!(
        username = %user.username,
        "Admin access denied: set OMNIVORE_ADMIN_USERS or OMNIVORE_GITHUB_ORG to grant it"
    );
    false
}

// -- Auth middleware --
// When OAuth is not configured, all requests pass through (open access).
// When OAuth IS configured, these enforce login and permission checks.

/// Middleware: require login to *view* anything, when the operator asks for it.
///
/// Coverage numbers, file paths, and hotspots are world-readable by default —
/// only mutations are gated. That is a reasonable default for an internal tool,
/// but file paths alone reveal a good deal about a private codebase, so
/// `OMNIVORE_REQUIRE_LOGIN_TO_VIEW=true` extends the login requirement over the
/// read-only pages and API too.
///
/// Health and auth endpoints stay open regardless: they are how a load balancer
/// checks liveness and how a user gets logged in.
pub async fn require_login_to_view_middleware(
    State(db): State<Database>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if !require_login_to_view() || OAuthConfig::from_env().is_none() {
        return next.run(request).await;
    }
    if extract_user(&db, &jar).await.is_some() {
        return next.run(request).await;
    }

    // An API client gets a status code it can act on; a browser gets sent to
    // the login page.
    let wants_html = request
        .headers()
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|accept| accept.contains("text/html"));

    if wants_html {
        Redirect::to("/auth/login").into_response()
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Is view access gated behind login?
pub fn require_login_to_view() -> bool {
    matches!(
        std::env::var("OMNIVORE_REQUIRE_LOGIN_TO_VIEW")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Middleware: require login when OAuth is enabled. Redirects to /auth/login if not authenticated.
pub async fn require_login_middleware(
    State(db): State<Database>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if OAuthConfig::from_env().is_none() {
        return next.run(request).await;
    }
    if extract_user(&db, &jar).await.is_some() {
        return next.run(request).await;
    }
    Redirect::to("/auth/login").into_response()
}

/// Middleware: require write access to the project named in the path.
///
/// Applied to every mutating project route. `require_login_middleware` alone
/// was not enough there: it let *any* logged-in GitHub user delete another
/// team's project, retarget its repository, or mint an API key scoped to it.
/// Write access means dashboard admin, or admin/maintain/write on the repo the
/// project is linked to.
///
/// A project with no linked repository has nothing to check permissions
/// against, so it is admin-only.
pub async fn require_project_write_middleware(
    State(db): State<Database>,
    Path(params): Path<std::collections::HashMap<String, String>>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if OAuthConfig::from_env().is_none() {
        return next.run(request).await;
    }

    let Some(project_id) = params.get("project_id") else {
        // Fail closed: this middleware is only mounted on routes that have a
        // {project_id}, so a missing one means a routing mistake.
        tracing::error!("require_project_write_middleware mounted on a route without {{project_id}}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    match require_project_write(&db, &jar, project_id).await {
        Ok(_) => next.run(request).await,
        Err(err) => err.into_response(),
    }
}

/// Middleware: require dashboard admin when OAuth is enabled.
pub async fn require_admin_middleware(
    State(db): State<Database>,
    jar: CookieJar,
    request: Request,
    next: Next,
) -> Response {
    if OAuthConfig::from_env().is_none() {
        return next.run(request).await;
    }
    match extract_user(&db, &jar).await {
        None => Redirect::to("/auth/login").into_response(),
        Some(user) => {
            if is_dashboard_admin(&db, &user).await {
                next.run(request).await
            } else {
                StatusCode::FORBIDDEN.into_response()
            }
        }
    }
}

// -- Auth guard helpers (for use in individual handlers) --
// When OAuth is not configured, these return Ok(None) to allow open access.
// When OAuth IS configured, they enforce login and permission checks.

/// Result type for auth guards. Ok(Some(user)) = authenticated, Ok(None) = OAuth not enabled (open),
/// Err(Redirect) = needs login.
pub type AuthResult = Result<Option<AuthUser>, Redirect>;

/// Require login when OAuth is enabled. Returns Ok(None) if OAuth is not configured.
pub async fn require_login(db: &Database, jar: &CookieJar) -> AuthResult {
    if OAuthConfig::from_env().is_none() {
        return Ok(None); // OAuth not configured — open access
    }
    match extract_user(db, jar).await {
        Some(user) => Ok(Some(user)),
        None => Err(Redirect::to("/auth/login")),
    }
}

/// Require dashboard admin. Returns Ok(None) if OAuth is not configured.
/// Returns Err(redirect to login) if not logged in.
/// Returns Ok(Some(user)) if admin, or StatusCode::FORBIDDEN.
pub async fn require_admin(db: &Database, jar: &CookieJar) -> Result<Option<AuthUser>, AuthGuardError> {
    if OAuthConfig::from_env().is_none() {
        return Ok(None);
    }
    let user = extract_user(db, jar).await
        .ok_or(AuthGuardError::Redirect(Redirect::to("/auth/login")))?;
    if is_dashboard_admin(db, &user).await {
        Ok(Some(user))
    } else {
        Err(AuthGuardError::Forbidden)
    }
}

/// Require admin/maintain on a project's linked repo (or dashboard admin).
/// Returns Ok(None) if OAuth is not configured.
pub async fn require_project_write(
    db: &Database,
    jar: &CookieJar,
    project_id: &str,
) -> Result<Option<AuthUser>, AuthGuardError> {
    if OAuthConfig::from_env().is_none() {
        return Ok(None);
    }
    let user = extract_user(db, jar).await
        .ok_or(AuthGuardError::Redirect(Redirect::to("/auth/login")))?;

    // Dashboard admins can do anything
    if is_dashboard_admin(db, &user).await {
        return Ok(Some(user));
    }

    // Check project's linked repo
    if let Ok(Some(project)) = db.get_project(project_id).await {
        if let Some(ref repo) = project.github_repo {
            if !repo.is_empty() {
                let perm = check_repo_permission(db, &user, repo).await;
                if perm == "admin" || perm == "maintain" || perm == "write" {
                    return Ok(Some(user));
                }
            }
        }
    }

    Err(AuthGuardError::Forbidden)
}

/// Error type for auth guards that need to distinguish redirect vs forbidden.
pub enum AuthGuardError {
    Redirect(Redirect),
    Forbidden,
}

impl axum::response::IntoResponse for AuthGuardError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthGuardError::Redirect(r) => r.into_response(),
            AuthGuardError::Forbidden => StatusCode::FORBIDDEN.into_response(),
        }
    }
}

async fn check_org_owner(token: &str, username: &str, org: &str) -> bool {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/orgs/{}/memberships/{}",
        org, username
    );

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "omnivore-dashboard")
        .send()
        .await;

    let resp = match resp {
        Ok(r) if r.status().is_success() => r,
        _ => return false,
    };

    #[derive(Deserialize)]
    struct MembershipResp {
        role: String,
    }

    resp.json::<MembershipResp>()
        .await
        .map(|m| m.role == "admin")
        .unwrap_or(false)
}
