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
    trend: Vec<TrendEntry>,
    files: Vec<FileCoverage>,
    has_dependencies: bool,
}

#[derive(serde::Serialize)]
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
    fn trend_json(&self) -> String {
        serde_json::to_string(&self.trend).unwrap_or_else(|_| "[]".to_string())
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
        items.push(ProjectWithLatest { project, latest });
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

    let snapshots = db
        .get_snapshots_for_project(&project_id, 30)
        .await
        .unwrap_or_default();

    let mut trend: Vec<TrendEntry> = snapshots
        .iter()
        .map(|s| TrendEntry {
            line_rate: s.line_rate,
            branch_rate: s.branch_rate,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();
    trend.reverse();

    let files: Vec<FileCoverage> = latest
        .as_ref()
        .and_then(|s| s.files_json.as_ref())
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    let has_dependencies = latest
        .as_ref()
        .and_then(|s| s.dependencies_json.as_ref())
        .is_some();

    let page = ProjectDetailPage {
        project,
        latest,
        trend,
        files,
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

pub async fn file_coverage_page(
    State(db): State<Database>,
    Path((project_id, file_path)): Path<(String, String)>,
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

    let files: Vec<FileCoverage> = latest
        .files_json
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default();

    let mut file = files
        .into_iter()
        .find(|f| f.path == file_path)
        .ok_or(StatusCode::NOT_FOUND)?;

    // If source not embedded in report, fetch from GitHub
    if file.source_content.is_none() {
        if let Some(repo) = &project.github_repo {
            let token = std::env::var("GITHUB_TOKEN").ok();
            // Prepend source_root to map JVM class paths to repo paths
            let repo_path = match &project.source_root {
                Some(root) => format!("{}/{}", root.trim_end_matches('/'), file_path),
                None => file_path.clone(),
            };
            let content = omnivore_core::github::source::fetch_source(
                repo,
                &repo_path,
                latest.commit_sha.as_deref(),
                token.as_deref(),
            )
            .await;
            file.source_content = content;
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
