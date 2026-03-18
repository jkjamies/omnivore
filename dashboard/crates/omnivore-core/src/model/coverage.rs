use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Omnivore report format — matches the JSON schema from the Kotlin plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmnivoreReport {
    pub version: String,
    pub format: String,
    pub project: ProjectInfo,
    pub coverage: CoverageSummary,
    pub files: Vec<FileCoverage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<DependencyGraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub target: CoverageTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageTarget {
    JvmUnit,
    AndroidInstrumented,
    IosUnit,
    KotlinNative,
    Composite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageSummary {
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
    pub path: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines: Vec<LineCoverage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineCoverage {
    pub line_number: i32,
    pub hit_count: i64,
}

// -- Dependency Graph --

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyGraph {
    pub modules: Vec<ModuleNode>,
    pub edges: Vec<ModuleEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub module_type: ModuleType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModuleType {
    Internal,
    External,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleEdge {
    pub from: String,
    pub to: String,
    pub configuration: String,
}

/// Stored coverage snapshot — what we persist in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSnapshot {
    pub id: String,
    pub project_id: String,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub target: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
    pub created_at: DateTime<Utc>,
    /// Full file-level coverage stored as JSON blob
    pub files_json: Option<String>,
    /// Dependency graph stored as JSON blob
    pub dependencies_json: Option<String>,
}
