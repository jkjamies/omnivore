use chrono::{DateTime, Utc};

/// A user session stored server-side in SQLite.
pub struct Session {
    pub id: String,
    pub github_username: String,
    pub github_token: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Cached repo permission for a user.
pub struct CachedPermission {
    pub user_id: String,
    pub repo: String,
    pub permission: String,
    pub expires_at: DateTime<Utc>,
}

/// The current authenticated user extracted from a session.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub username: String,
    pub github_token: String,
    pub avatar_url: Option<String>,
}
