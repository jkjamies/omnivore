use axum::extract::{Query, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use omnivore_core::model::session::AuthUser;
use omnivore_core::storage::Database;
use serde::{Deserialize, Serialize};

const SESSION_COOKIE: &str = "omnivore_session";

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
pub async fn login(State(config): State<OAuthConfig>) -> Redirect {
    let scopes = "read:user,read:org,repo";
    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&scope={}",
        config.client_id, scopes
    );
    Redirect::temporary(&url)
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
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

/// Check if user is a dashboard admin.
/// - If OMNIVORE_GITHUB_ORG is set: org owners = admin
/// - Otherwise: admin on any linked repo = admin
pub async fn is_dashboard_admin(db: &Database, user: &AuthUser) -> bool {
    // Strategy 1: org-based
    if let Ok(org) = std::env::var("OMNIVORE_GITHUB_ORG") {
        if !org.is_empty() {
            return check_org_owner(&user.github_token, &user.username, &org).await;
        }
    }

    // Strategy 2: admin on any linked project repo
    let projects = db.list_projects().await.unwrap_or_default();
    for project in &projects {
        if let Some(ref repo) = project.github_repo {
            if !repo.is_empty() {
                let perm = check_repo_permission(db, user, repo).await;
                if perm == "admin" || perm == "maintain" {
                    return true;
                }
            }
        }
    }

    false
}

// -- Auth middleware --
// When OAuth is not configured, all requests pass through (open access).
// When OAuth IS configured, these enforce login and permission checks.

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
