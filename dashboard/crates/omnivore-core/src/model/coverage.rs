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

/// The largest line number any real source file can plausibly have.
///
/// Coverage line numbers are `i32` and, until this bound existed, were taken
/// entirely on trust from the uploaded report. Nothing rejected
/// `{"lineNumber": 2000000000}`, and nothing downstream expected it — the file
/// coverage page renders a row per line from 1 to the highest line it sees, so
/// a single such record turned every later *read* of that page into an attempt
/// to allocate two billion rows. Ingest is unauthenticated on a default
/// install, the record is persisted, and the victim is whoever opens the page
/// next. That made a one-line upload a durable denial of service.
///
/// Two million is far above any real file (the largest generated sources in the
/// wild are low hundreds of thousands of lines) and far below the point where
/// anything downstream struggles.
pub const MAX_LINE_NUMBER: i32 = 2_000_000;

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
    /// Branch edges covered / total for this file.
    ///
    /// Required to aggregate branch coverage correctly. Rolling a directory or
    /// project rate up from per-file `branch_rate` values yields an unweighted
    /// mean, in which a 3-branch file counts as much as a 300-branch one — see
    /// [`FileCoverage::branch_totals`].
    ///
    /// Defaults to 0 so reports from older producers still deserialize; use
    /// [`FileCoverage::has_branch_counts`] to tell "no branches" from "producer
    /// didn't tell us".
    #[serde(default)]
    pub branches_covered: i64,
    #[serde(default)]
    pub branches_total: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_content: Option<String>,
}

impl FileCoverage {
    /// Whether this file carries real branch counts.
    ///
    /// A file legitimately without branches and a file from a producer that
    /// never reported counts both show `branches_total == 0`. They are only
    /// distinguishable by whether *any* file in the snapshot has counts, which
    /// is what [`branch_totals`](Self::branch_totals) handles for a collection.
    pub fn has_branch_counts(&self) -> bool {
        self.branches_total > 0
    }

    /// Sum `(covered, total)` branch edges across files.
    ///
    /// Returns `None` when no file reports any branches, which means the
    /// producer predates per-file branch counts — callers should then fall back
    /// to the snapshot-level totals rather than silently reporting 0%.
    pub fn branch_totals(files: &[FileCoverage]) -> Option<(i64, i64)> {
        let covered: i64 = files.iter().map(|f| f.branches_covered).sum();
        let total: i64 = files.iter().map(|f| f.branches_total).sum();
        if total > 0 { Some((covered, total)) } else { None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineCoverage {
    pub line_number: i32,
    pub hit_count: i64,
}

impl OmnivoreReport {
    /// Discard line records that cannot describe a real source line.
    ///
    /// Returns how many were dropped, so the caller can say so rather than
    /// silently changing the numbers.
    ///
    /// A line number outside `1..=MAX_LINE_NUMBER` is not "unusual coverage" —
    /// it is malformed or hostile input, and there is no reading of it that
    /// produces a useful report. Dropping is preferable to rejecting the whole
    /// upload: a genuinely broken producer should still get the coverage it
    /// measured correctly, and an attacker gets nothing either way.
    pub fn drop_implausible_lines(&mut self) -> usize {
        let mut dropped = 0;
        for file in &mut self.files {
            let before = file.lines.len();
            file.lines
                .retain(|l| l.line_number >= 1 && l.line_number <= MAX_LINE_NUMBER);
            dropped += before - file.lines.len();
        }
        if dropped > 0 {
            tracing::warn!(
                dropped,
                max_line_number = MAX_LINE_NUMBER,
                "Ignored coverage records with implausible line numbers"
            );
        }
        dropped
    }
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
    /// Build a snapshot from a parsed report.
    ///
    /// Takes `&mut` so it can sanitize first. Every parser funnels through here
    /// on its way to storage, which makes this the one place a bound on line
    /// numbers cannot be forgotten when a seventh format is added — and the
    /// place it has to be, since `files_json` is serialized here and is what the
    /// file coverage page later reads back.
    pub fn from_report(report: &mut OmnivoreReport, source: Option<&str>) -> Self {
        report.drop_implausible_lines();

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
