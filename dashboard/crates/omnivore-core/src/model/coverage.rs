use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Canonical provenance identifiers — which tool produced the coverage data.
///
/// This is orthogonal to [`CoverageTarget`]: `target` describes *where/how* the
/// code ran (JVM unit tests, instrumented, iOS…), while `source` records *who
/// measured it* (the Omnivore agent, Kover, JaCoCo, llvm-cov…). Keeping the two
/// separate lets a project host coverage from several tools without conflating
/// their trends.
pub mod source {
    pub const OMNIVORE_AGENT: &str = "omnivore-agent";
    pub const KOVER: &str = "kover";
    pub const JACOCO: &str = "jacoco";
    pub const LLVM_COV: &str = "llvm-cov";
    pub const LCOV: &str = "lcov";
    pub const GO: &str = "go";
    pub const PYTHON_COVERAGE: &str = "python-coverage";
}

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
    /// Provenance — which tool produced this report (see [`source`]). Optional in
    /// the wire format; defaults to the Omnivore agent when a report omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageTarget {
    JvmUnit,
    AndroidInstrumented,
    IosUnit,
    KotlinNative,
    Composite,
    RustLlvmCov,
    GoCover,
    PythonCoverage,
    Lcov,
}

impl CoverageTarget {
    /// Canonical `SCREAMING_SNAKE_CASE` string — matches the serde representation
    /// and the values documented for the `target` column. Use this when persisting
    /// or comparing targets so stored values stay consistent (rather than the
    /// `Debug`/PascalCase form).
    pub fn as_str(&self) -> &'static str {
        match self {
            CoverageTarget::JvmUnit => "JVM_UNIT",
            CoverageTarget::AndroidInstrumented => "ANDROID_INSTRUMENTED",
            CoverageTarget::IosUnit => "IOS_UNIT",
            CoverageTarget::KotlinNative => "KOTLIN_NATIVE",
            CoverageTarget::Composite => "COMPOSITE",
            CoverageTarget::RustLlvmCov => "RUST_LLVM_COV",
            CoverageTarget::GoCover => "GO_COVER",
            CoverageTarget::PythonCoverage => "PYTHON_COVERAGE",
            CoverageTarget::Lcov => "LCOV",
        }
    }

    /// Parse a target from a user-supplied string (case-insensitive, accepts the
    /// canonical `SCREAMING_SNAKE_CASE` or the PascalCase `Debug` form). Used for
    /// the optional `?target=` ingest override.
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_uppercase().replace('-', "_").as_str() {
            "JVM_UNIT" | "JVMUNIT" => Some(CoverageTarget::JvmUnit),
            "ANDROID_INSTRUMENTED" | "ANDROIDINSTRUMENTED" => Some(CoverageTarget::AndroidInstrumented),
            "IOS_UNIT" | "IOSUNIT" => Some(CoverageTarget::IosUnit),
            "KOTLIN_NATIVE" | "KOTLINNATIVE" => Some(CoverageTarget::KotlinNative),
            "COMPOSITE" => Some(CoverageTarget::Composite),
            "RUST_LLVM_COV" | "RUSTLLVMCOV" => Some(CoverageTarget::RustLlvmCov),
            "GO_COVER" | "GOCOVER" => Some(CoverageTarget::GoCover),
            "PYTHON_COVERAGE" | "PYTHONCOVERAGE" => Some(CoverageTarget::PythonCoverage),
            "LCOV" => Some(CoverageTarget::Lcov),
            _ => None,
        }
    }
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
    /// Provenance — which tool produced this snapshot (see [`source`]).
    pub source: String,
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

impl CoverageSnapshot {
    /// Build a storable snapshot from a normalized report.
    ///
    /// Every parser funnels through here so snapshot construction lives in one
    /// place: the `target` is persisted in its canonical `SCREAMING_SNAKE_CASE`
    /// form (via [`CoverageTarget::as_str`]) and `source` records provenance.
    /// A `None`/empty `source` argument falls back to the report's own
    /// `project.source`, then to the Omnivore agent.
    pub fn from_report(report: &OmnivoreReport, source: Option<&str>) -> Self {
        let files_json = serde_json::to_string(&report.files).ok();
        let dependencies_json = report
            .dependencies
            .as_ref()
            .and_then(|d| serde_json::to_string(d).ok());

        let source = source
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| report.project.source.clone().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| source::OMNIVORE_AGENT.to_string());

        CoverageSnapshot {
            id: Uuid::new_v4().to_string(),
            project_id: report.project.id.clone(),
            commit_sha: report.project.commit_sha.clone(),
            branch: report.project.branch.clone(),
            target: report.project.target.as_str().to_string(),
            source,
            line_rate: report.coverage.line_rate,
            branch_rate: report.coverage.branch_rate,
            lines_covered: report.coverage.lines_covered,
            lines_total: report.coverage.lines_total,
            branches_covered: report.coverage.branches_covered,
            branches_total: report.coverage.branches_total,
            file_count: report.files.len() as i64,
            created_at: Utc::now(),
            files_json,
            dependencies_json,
        }
    }
}
