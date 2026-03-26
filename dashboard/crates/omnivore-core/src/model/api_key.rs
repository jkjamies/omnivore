use chrono::{DateTime, Utc};

/// An API key stored in the database (never contains the raw key).
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub project_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Returned once on creation, containing the full plaintext key.
pub struct ApiKeyCreated {
    pub id: String,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub project_id: Option<String>,
}
