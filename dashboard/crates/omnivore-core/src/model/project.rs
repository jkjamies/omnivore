use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A project registered in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_repo: Option<String>,
    /// Path prefix from repo root to source files (e.g., "src/main/kotlin" or "app/src/main/kotlin").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_warn_threshold: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_warn_threshold: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateProject {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub github_repo: Option<String>,
    #[serde(default)]
    pub source_root: Option<String>,
    #[serde(default)]
    pub line_threshold: Option<f64>,
    #[serde(default)]
    pub branch_threshold: Option<f64>,
    #[serde(default)]
    pub line_warn_threshold: Option<f64>,
    #[serde(default)]
    pub branch_warn_threshold: Option<f64>,
}
