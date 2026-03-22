use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::{CoverageSnapshot, DependencyGraph, FileCoverage};
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

// -- Directory tree for file breakdown --

/// A node in the file tree: either a directory (with children) or a leaf file.
pub struct FileTreeNode {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
    /// Aggregate line coverage rate for directories, file rate for leaves.
    pub line_rate: f64,
    pub branch_rate: f64,
    pub file_count: usize,
}

/// Build a nested directory tree from a flat list of file coverages.
fn build_file_tree(files: &[FileCoverage]) -> Vec<FileTreeNode> {
    use std::collections::BTreeMap;

    // Group files by their first path component
    let mut groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    let mut root_files: Vec<&FileCoverage> = Vec::new();

    for f in files {
        if let Some(idx) = f.path.find('/') {
            let dir = f.path[..idx].to_string();
            groups.entry(dir).or_default().push(f);
        } else {
            root_files.push(f);
        }
    }

    let mut nodes = Vec::new();

    // Directories first
    for (dir_name, dir_files) in &groups {
        nodes.push(build_dir_node(dir_name, "", dir_files));
    }

    // Then root-level files
    for f in root_files {
        nodes.push(FileTreeNode {
            name: f.path.clone(),
            full_path: f.path.clone(),
            is_dir: false,
            children: vec![],
            line_rate: f.line_rate,
            branch_rate: f.branch_rate,
            file_count: 1,
        });
    }

    nodes
}

fn build_dir_node(dir_name: &str, parent_path: &str, files: &[&FileCoverage]) -> FileTreeNode {
    use std::collections::BTreeMap;

    let full_dir = if parent_path.is_empty() {
        dir_name.to_string()
    } else {
        format!("{}/{}", parent_path, dir_name)
    };
    let prefix = format!("{}/", full_dir);

    // Strip the prefix from file paths and regroup
    let mut sub_groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    let mut leaf_files: Vec<&FileCoverage> = Vec::new();

    for f in files {
        let relative = f.path.strip_prefix(&prefix).unwrap_or(&f.path);
        if let Some(idx) = relative.find('/') {
            let sub_dir = relative[..idx].to_string();
            sub_groups.entry(sub_dir).or_default().push(f);
        } else {
            leaf_files.push(f);
        }
    }

    // If there's exactly one subdirectory and no leaf files, collapse it
    // e.g., "com" → "example" → "app" becomes "com/example/app"
    if sub_groups.len() == 1 && leaf_files.is_empty() {
        let (sub_name, sub_files) = sub_groups.into_iter().next().unwrap();
        let collapsed_name = format!("{}/{}", dir_name, sub_name);
        return build_dir_node(&collapsed_name, parent_path, &sub_files);
    }

    let mut children = Vec::new();

    for (sub_name, sub_files) in &sub_groups {
        children.push(build_dir_node(sub_name, &full_dir, sub_files));
    }

    for f in &leaf_files {
        let file_name = f.path.rsplit('/').next().unwrap_or(&f.path).to_string();
        children.push(FileTreeNode {
            name: file_name,
            full_path: f.path.clone(),
            is_dir: false,
            children: vec![],
            line_rate: f.line_rate,
            branch_rate: f.branch_rate,
            file_count: 1,
        });
    }

    // Aggregate coverage for the directory
    let total_lines: i64 = files.iter().map(|f| f.lines.len() as i64).sum();
    let covered_lines: i64 = files.iter().map(|f| f.lines.iter().filter(|l| l.hit_count > 0).count() as i64).sum();
    let line_rate = if total_lines > 0 { covered_lines as f64 / total_lines as f64 } else { 0.0 };

    let total_branches: f64 = files.len() as f64;
    let branch_rate_sum: f64 = files.iter().map(|f| f.branch_rate).sum();
    let branch_rate = if total_branches > 0.0 { branch_rate_sum / total_branches } else { 0.0 };

    FileTreeNode {
        name: dir_name.to_string(),
        full_path: full_dir,
        is_dir: true,
        children,
        line_rate,
        branch_rate,
        file_count: files.len(),
    }
}

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
    pub file_tree: Vec<FileTreeNode>,
    pub trend: Vec<TrendEntry>,
}

impl TargetSnapshot {
    fn from_snapshot(snap: &CoverageSnapshot, trend: Vec<TrendEntry>) -> Self {
        let files: Vec<FileCoverage> = snap
            .files_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        let file_tree = build_file_tree(&files);
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
            file_tree,
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

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
    /// Render a file tree as flat HTML table rows with data attributes for JS toggle.
    fn render_file_tree(&self, nodes: &[FileTreeNode], depth: usize) -> String {
        let mut html = String::new();
        self.render_file_tree_inner(nodes, depth, "", &mut html);
        html
    }

    fn render_file_tree_inner(&self, nodes: &[FileTreeNode], depth: usize, parent_id: &str, html: &mut String) {
        for node in nodes {
            let line_color = rate_color_val(node.line_rate);
            let branch_color = rate_color_val(node.branch_rate);
            let indent_px = depth * 20 + 12;

            if node.is_dir {
                // Use the full_path as a stable ID for parent-child linking
                let dir_id = html_escape(&node.full_path);
                let hidden = if depth == 0 { "" } else { " style=\"display:none;\"" };
                html.push_str(&format!(
                    r#"<tr class="tree-dir" data-depth="{depth}" data-dir="{dir_id}" data-parent="{parent_id}"{hidden} onclick="toggleDir(this, '{dir_id}')">"#,
                ));
                html.push_str(&format!(
                    r#"<td style="padding-left:{}px;cursor:pointer;"><span class="tree-arrow" data-dir="{dir_id}">&#x25B6;</span> <span class="tree-icon">&#x1F4C1;</span> <strong>{}</strong> <span class="tree-count">({} files)</span></td>"#,
                    indent_px,
                    html_escape(&node.name),
                    node.file_count,
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span></td>"#,
                    line_color, fmt_pct_val(node.line_rate),
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span></td>"#,
                    branch_color, fmt_pct_val(node.branch_rate),
                ));
                html.push_str(&format!(
                    r#"<td><div class="coverage-bar"><div class="coverage-bar-fill" style="width:{}%;--rate:{:.4};"></div></div></td>"#,
                    fmt_pct_val(node.line_rate), node.line_rate,
                ));
                html.push_str("</tr>");
                // Recurse children — they reference this dir as parent
                self.render_file_tree_inner(&node.children, depth + 1, &dir_id, html);
            } else {
                let hidden = if depth == 0 { "" } else { " style=\"display:none;\"" };
                html.push_str(&format!(
                    r#"<tr class="tree-file" data-depth="{depth}" data-parent="{parent_id}"{hidden}>"#,
                ));
                html.push_str(&format!(
                    r#"<td style="padding-left:{}px;"><a href="/projects/{}/files/{}" class="file-path">{}</a></td>"#,
                    indent_px,
                    html_escape(&self.project.id),
                    html_escape(&node.full_path),
                    html_escape(&node.name),
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span></td>"#,
                    line_color, fmt_pct_val(node.line_rate),
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span></td>"#,
                    branch_color, fmt_pct_val(node.branch_rate),
                ));
                html.push_str(&format!(
                    r#"<td><div class="coverage-bar"><div class="coverage-bar-fill" style="width:{}%;--rate:{:.4};"></div></div></td>"#,
                    fmt_pct_val(node.line_rate), node.line_rate,
                ));
                html.push_str("</tr>");
            }
        }
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

/// Renders the file coverage page immediately with coverage data only.
/// Source code is loaded asynchronously via HTMX from the source endpoint.
pub async fn file_coverage_page(
    State(db): State<Database>,
    Path((project_id, file_path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let file = find_file_across_targets(&db, &project_id, &file_path).await?;

    let page = FileCoveragePage { project, file_path, file };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

/// HTMX endpoint: fetches source code (from DB cache or GitHub) and returns
/// an HTML fragment with the source table rows including code content.
pub async fn file_source_fragment(
    State(db): State<Database>,
    Path((project_id, file_path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut file = find_file_across_targets(&db, &project_id, &file_path).await?;

    // Try DB cache first
    if let Some(repo) = &project.github_repo {
        let commit_ref = ""; // TODO: could use snapshot commit_sha
        if let Ok(Some(cached)) = db.get_cached_source(repo, &file_path, commit_ref).await {
            file.source_content = Some(cached);
        } else {
            // Fetch from GitHub (raw URL) and cache
            let token = std::env::var("GITHUB_TOKEN").ok();
            let candidates = build_source_candidates(
                project.source_root.as_deref(),
                &file_path,
            );

            for candidate in &candidates {
                let content = omnivore_core::github::source::fetch_source(
                    repo,
                    candidate,
                    None,
                    token.as_deref(),
                )
                .await;
                if let Some(src) = content {
                    // Cache it
                    let _ = db.cache_source(repo, &file_path, commit_ref, &src).await;
                    file.source_content = Some(src);
                    break;
                }
            }
        }
    }

    // Render just the table rows as an HTML fragment
    let page = FileCoveragePage {
        project,
        file_path,
        file,
    };
    let rows = page.line_rows();
    let mut html = String::new();
    for line in &rows {
        html.push_str(&format!(
            r#"<tr class="{}"><td class="line-gutter">{}</td><td class="line-num">{}</td><td class="line-hits">{}</td><td class="line-code"><pre>{}</pre></td></tr>"#,
            line.css_class,
            match line.status {
                LineStatus::Covered => r#"<span class="gutter-mark gutter-covered"></span>"#,
                LineStatus::Uncovered => r#"<span class="gutter-mark gutter-uncovered"></span>"#,
                LineStatus::None => "",
            },
            line.number,
            match line.status {
                LineStatus::Covered => format!(r#"<span class="hit-badge hit-covered">{}x</span>"#, line.hits),
                LineStatus::Uncovered => r#"<span class="hit-badge hit-uncovered">0x</span>"#.to_string(),
                LineStatus::None => String::new(),
            },
            html_escape(&line.code),
        ));
    }
    Ok(Html(html))
}

/// Find a file's coverage data across all target snapshots for a project.
async fn find_file_across_targets(
    db: &Database,
    project_id: &str,
    file_path: &str,
) -> Result<FileCoverage, StatusCode> {
    let target_names = db
        .get_targets_for_project(project_id)
        .await
        .unwrap_or_default();

    for tname in &target_names {
        if let Ok(Some(snap)) = db.get_latest_snapshot_by_target(project_id, tname).await {
            let files: Vec<FileCoverage> = snap
                .files_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            if let Some(f) = files.into_iter().find(|f| f.path == file_path) {
                return Ok(f);
            }
        }
    }

    Err(StatusCode::NOT_FOUND)
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
