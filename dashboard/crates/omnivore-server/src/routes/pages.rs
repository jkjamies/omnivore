use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::{CoverageSnapshot, DependencyGraph, FileCoverage};
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

// -- Helper types --

pub struct ProjectWithLatest {
    pub project: Project,
    pub latest: Option<CoverageSnapshot>,
    /// Per-target latest snapshots (e.g., JVM_UNIT, ANDROID_INSTRUMENTED)
    pub targets: Vec<TargetSnapshot>,
}

/// Summary data for a single coverage target.
pub struct TargetSnapshot {
    pub target: String,
    pub label: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
    pub files: Vec<FileCoverage>,
    pub trend: Vec<TrendEntry>,
}

impl TargetSnapshot {
    fn from_snapshot(snap: &CoverageSnapshot, trend: Vec<TrendEntry>) -> Self {
        let files: Vec<FileCoverage> = snap
            .files_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        Self {
            target: snap.target.clone(),
            label: target_label(&snap.target),
            line_rate: snap.line_rate,
            branch_rate: snap.branch_rate,
            lines_covered: snap.lines_covered,
            lines_total: snap.lines_total,
            branches_covered: snap.branches_covered,
            branches_total: snap.branches_total,
            file_count: snap.file_count,
            files,
            trend,
        }
    }
}

/// Composite summary computed from multiple targets.
pub struct CompositeSnapshot {
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
}

fn compute_composite(targets: &[TargetSnapshot]) -> Option<CompositeSnapshot> {
    if targets.len() < 2 {
        return None;
    }
    let lines_covered: i64 = targets.iter().map(|t| t.lines_covered).sum();
    let lines_total: i64 = targets.iter().map(|t| t.lines_total).sum();
    let branches_covered: i64 = targets.iter().map(|t| t.branches_covered).sum();
    let branches_total: i64 = targets.iter().map(|t| t.branches_total).sum();
    let file_count: i64 = targets.iter().map(|t| t.file_count).sum();
    let line_rate = if lines_total > 0 { lines_covered as f64 / lines_total as f64 } else { 0.0 };
    let branch_rate = if branches_total > 0 { branches_covered as f64 / branches_total as f64 } else { 0.0 };
    Some(CompositeSnapshot {
        line_rate,
        branch_rate,
        lines_covered,
        lines_total,
        branches_covered,
        branches_total,
        file_count,
    })
}

fn target_label(target: &str) -> String {
    match target {
        "JVM_UNIT" | "JvmUnit" => "Unit Tests".to_string(),
        "ANDROID_INSTRUMENTED" | "AndroidInstrumented" => "Instrumented Tests".to_string(),
        "IOS_UNIT" | "IosUnit" => "iOS Unit Tests".to_string(),
        "KOTLIN_NATIVE" | "KotlinNative" => "Kotlin/Native Tests".to_string(),
        "COMPOSITE" | "Composite" => "Composite".to_string(),
        other => other.to_string(),
    }
}

// -- Shared helpers --

fn fmt_pct_val(rate: f64) -> String {
    format!("{:.1}", rate * 100.0)
}

fn rate_color_val(rate: f64) -> &'static str {
    if rate >= 0.8 {
        "var(--green)"
    } else if rate >= 0.5 {
        "var(--yellow)"
    } else {
        "var(--red)"
    }
}

// -- Templates --

#[derive(Template)]
#[template(path = "projects.html")]
pub struct ProjectsPage {
    projects: Vec<ProjectWithLatest>,
}

impl ProjectsPage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color(&self, rate: &f64) -> &'static str {
        rate_color_val(*rate)
    }
}

#[derive(Template)]
#[template(path = "project_detail.html")]
pub struct ProjectDetailPage {
    project: Project,
    latest: Option<CoverageSnapshot>,
    targets: Vec<TargetSnapshot>,
    composite: Option<CompositeSnapshot>,
    has_dependencies: bool,
}

#[derive(serde::Serialize, Clone)]
pub struct TrendEntry {
    pub line_rate: f64,
    pub branch_rate: f64,
    pub created_at: String,
}

impl ProjectDetailPage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color(&self, rate: &f64) -> &'static str {
        rate_color_val(*rate)
    }
    fn short_sha<'a>(&self, sha: &'a str) -> &'a str {
        if sha.len() > 7 { &sha[..7] } else { sha }
    }
    fn has_trend(&self) -> bool {
        self.targets.iter().any(|t| t.trend.len() > 1)
    }
    fn all_trends_json(&self) -> String {
        let datasets: Vec<serde_json::Value> = self.targets.iter().map(|t| {
            serde_json::json!({
                "label": t.label,
                "target": t.target,
                "data": t.trend,
            })
        }).collect();
        serde_json::to_string(&datasets).unwrap_or_else(|_| "[]".to_string())
    }
}

// -- Handlers --

pub async fn projects_page(
    State(db): State<Database>,
) -> Result<Html<String>, StatusCode> {
    let projects = db
        .list_projects()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut items = Vec::with_capacity(projects.len());
    for project in projects {
        let latest = db
            .get_latest_snapshot(&project.id)
            .await
            .unwrap_or(None);

        // Fetch per-target latest for project cards
        let target_names = db
            .get_targets_for_project(&project.id)
            .await
            .unwrap_or_default();
        let mut targets = Vec::new();
        for tname in &target_names {
            if let Ok(Some(snap)) = db.get_latest_snapshot_by_target(&project.id, tname).await {
                targets.push(TargetSnapshot::from_snapshot(&snap, vec![]));
            }
        }

        items.push(ProjectWithLatest { project, latest, targets });
    }

    let page = ProjectsPage { projects: items };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn project_detail_page(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let latest = db
        .get_latest_snapshot(&project_id)
        .await
        .unwrap_or(None);

    // Fetch per-target data
    let target_names = db
        .get_targets_for_project(&project_id)
        .await
        .unwrap_or_default();

    let mut targets = Vec::new();
    for tname in &target_names {
        if let Ok(Some(snap)) = db.get_latest_snapshot_by_target(&project_id, tname).await {
            let snaps = db
                .get_snapshots_for_project_by_target(&project_id, tname, 30)
                .await
                .unwrap_or_default();
            let mut trend: Vec<TrendEntry> = snaps
                .iter()
                .map(|s| TrendEntry {
                    line_rate: s.line_rate,
                    branch_rate: s.branch_rate,
                    created_at: s.created_at.to_rfc3339(),
                })
                .collect();
            trend.reverse();
            targets.push(TargetSnapshot::from_snapshot(&snap, trend));
        }
    }

    let composite = compute_composite(&targets);

    let has_dependencies = latest
        .as_ref()
        .and_then(|s| s.dependencies_json.as_ref())
        .is_some();

    let page = ProjectDetailPage {
        project,
        latest,
        targets,
        composite,
        has_dependencies,
    };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// -- File Coverage Page --

#[derive(Debug, Clone, PartialEq)]
pub enum LineStatus {
    Covered,
    Uncovered,
    None,
}

pub struct LineRow {
    pub number: i32,
    pub hits: i64,
    pub status: LineStatus,
    pub css_class: &'static str,
    pub code: String,
}

#[derive(Template)]
#[template(path = "file_coverage.html")]
pub struct FileCoveragePage {
    project: Project,
    file_path: String,
    file: FileCoverage,
}

impl FileCoveragePage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color(&self, rate: &f64) -> &'static str {
        rate_color_val(*rate)
    }
    fn covered_count(&self) -> usize {
        self.file.lines.iter().filter(|l| l.hit_count > 0).count()
    }
    fn total_count(&self) -> usize {
        self.file.lines.len()
    }
    fn has_source(&self) -> bool {
        self.file.source_content.is_some()
    }
    fn line_rows(&self) -> Vec<LineRow> {
        let mut hit_map = std::collections::HashMap::new();
        for line in &self.file.lines {
            hit_map.insert(line.line_number, line.hit_count);
        }

        let source_lines: Vec<&str> = self.file.source_content
            .as_deref()
            .map(|s| s.lines().collect())
            .unwrap_or_default();

        let max_line = if !source_lines.is_empty() {
            source_lines.len() as i32
        } else if self.file.lines.is_empty() {
            return vec![];
        } else {
            self.file.lines.iter().map(|l| l.line_number).max().unwrap_or(1)
        };

        (1..=max_line)
            .map(|n| {
                let code = source_lines
                    .get((n - 1) as usize)
                    .unwrap_or(&"")
                    .to_string();

                if let Some(&hits) = hit_map.get(&n) {
                    if hits > 0 {
                        LineRow { number: n, hits, status: LineStatus::Covered, css_class: "line-covered", code }
                    } else {
                        LineRow { number: n, hits: 0, status: LineStatus::Uncovered, css_class: "line-uncovered", code }
                    }
                } else {
                    LineRow { number: n, hits: 0, status: LineStatus::None, css_class: "", code }
                }
            })
            .collect()
    }
}

/// Build candidate GitHub paths for a JVM class file path.
///
/// For multi-module Gradle projects, the coverage report stores paths like
/// `com/example/app/domain/model/Task.kt`. The actual repo path depends on
/// which module the file lives in. We try common Gradle source layouts:
/// `{source_root}/{module}/src/main/{java,kotlin}/{file_path}`
///
/// We also try the direct path and the source_root as a prefix.
fn build_source_candidates(source_root: Option<&str>, file_path: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let src_dirs = ["src/main/java", "src/main/kotlin"];

    if let Some(root) = source_root {
        let root = root.trim_end_matches('/');

        // If source_root already contains src/main/java or similar, use it directly
        if src_dirs.iter().any(|d| root.contains(d)) {
            candidates.push(format!("{}/{}", root, file_path));
        }

        // Try each segment of the file path as a potential module name
        // e.g., for path "com/example/testrig/domain/model/Task.kt"
        // try: {root}/domain/src/main/java/com/example/testrig/domain/model/Task.kt
        let segments: Vec<&str> = file_path.split('/').collect();
        let mut seen = std::collections::HashSet::new();
        for seg in &segments {
            if seen.insert(*seg) {
                for src_dir in &src_dirs {
                    candidates.push(format!("{}/{}/{}/{}", root, seg, src_dir, file_path));
                }
            }
        }

        // Also try with just the root as prefix
        candidates.push(format!("{}/{}", root, file_path));
    }

    // Fallback: bare file path
    candidates.push(file_path.to_string());
    candidates
}

pub async fn file_coverage_page(
    State(db): State<Database>,
    Path((project_id, file_path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Search across all target snapshots for the requested file
    let target_names = db
        .get_targets_for_project(&project_id)
        .await
        .unwrap_or_default();

    let mut file: Option<FileCoverage> = None;
    let mut matched_snapshot: Option<CoverageSnapshot> = None;

    for tname in &target_names {
        if let Ok(Some(snap)) = db.get_latest_snapshot_by_target(&project_id, tname).await {
            let files: Vec<FileCoverage> = snap
                .files_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            if let Some(f) = files.into_iter().find(|f| f.path == file_path) {
                matched_snapshot = Some(snap);
                file = Some(f);
                break;
            }
        }
    }

    let mut file = file.ok_or(StatusCode::NOT_FOUND)?;
    let snapshot = matched_snapshot.unwrap();

    // If source not embedded in report, fetch from GitHub
    if file.source_content.is_none() {
        if let Some(repo) = &project.github_repo {
            let token = std::env::var("GITHUB_TOKEN").ok();
            let sha = snapshot.commit_sha.as_deref();

            // Build candidate paths to try. For multi-module Gradle/Android projects,
            // source_root is the project base (e.g., "test-rigs/android-test-rig").
            // We try: source_root + common Gradle src dirs for each submodule inferred
            // from the package path, plus the exact configured source_root.
            let candidates = build_source_candidates(
                project.source_root.as_deref(),
                &file_path,
            );

            for candidate in &candidates {
                let content = omnivore_core::github::source::fetch_source(
                    repo,
                    candidate,
                    sha,
                    token.as_deref(),
                )
                .await;
                if content.is_some() {
                    file.source_content = content;
                    break;
                }
            }
        }
    }

    let page = FileCoveragePage { project, file_path, file };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// -- Dependency Graph Page --

#[derive(Template)]
#[template(path = "dependency_graph.html")]
pub struct DependencyGraphPage {
    project: Project,
    graph: DependencyGraph,
}

impl DependencyGraphPage {
    fn graph_json(&self) -> String {
        serde_json::to_string(&self.graph).unwrap_or_else(|_| r#"{"modules":[],"edges":[]}"#.to_string())
    }
    fn internal_count(&self) -> usize {
        use omnivore_core::model::coverage::ModuleType;
        self.graph.modules.iter().filter(|m| m.module_type == ModuleType::Internal).count()
    }
}

pub async fn dependency_graph_page(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let latest = db
        .get_latest_snapshot(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let graph: DependencyGraph = latest
        .dependencies_json
        .as_deref()
        .ok_or(StatusCode::NOT_FOUND)
        .and_then(|json| serde_json::from_str(json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR))?;

    let page = DependencyGraphPage { project, graph };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}
