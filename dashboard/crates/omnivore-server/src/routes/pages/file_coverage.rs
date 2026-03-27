use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum_extra::extract::cookie::CookieJar;
use omnivore_core::model::coverage::FileCoverage;
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

use super::{fmt_delta_html, fmt_pct_val, html_escape, rate_color_val};
use crate::routes::auth;

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
struct FileCoveragePage {
    project: Project,
    file_path: String,
    file: FileCoverage,
    line_delta: Option<f64>,
    branch_delta: Option<f64>,
}

impl FileCoveragePage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color(&self, rate: &f64) -> &'static str {
        rate_color_val(*rate)
    }
    fn fmt_delta(&self, delta: &Option<f64>) -> String {
        fmt_delta_html(*delta)
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

pub async fn file_coverage_page(
    State(db): State<Database>,
    Path((project_id, file_path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let (file, line_delta, branch_delta) = find_file_with_delta(&db, &project_id, &file_path).await?;

    let page = FileCoveragePage { project, file_path, file, line_delta, branch_delta };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn file_source_fragment(
    State(db): State<Database>,
    jar: CookieJar,
    Path((project_id, file_path)): Path<(String, String)>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut file = find_file_across_targets(&db, &project_id, &file_path).await?;

    if let Some(repo) = &project.github_repo {
        let commit_ref = "";
        if let Ok(Some(cached)) = db.get_cached_source(repo, &file_path, commit_ref).await {
            file.source_content = Some(cached);
        } else {
            // Prefer the logged-in user's GitHub token, fall back to server GITHUB_TOKEN
            let user = auth::extract_user(&db, &jar).await;
            let user_token = user.as_ref().map(|u| u.github_token.clone());
            let env_token = std::env::var("GITHUB_TOKEN").ok();
            let effective_token = user_token.as_deref().or(env_token.as_deref());

            if let Some(ref u) = user {
                tracing::info!(username = %u.username, "Source fetch using logged-in user's token");
            } else if env_token.is_some() {
                tracing::info!("Source fetch using server GITHUB_TOKEN");
            } else {
                tracing::info!("Source fetch with no token (public repos only)");
            }

            // Resolve the coverage path to the actual repo path via Git Trees API
            let resolved = omnivore_core::github::source::resolve_file_path(
                repo,
                &file_path,
                effective_token,
            )
            .await;

            if let Some(repo_path) = resolved {
                let content = omnivore_core::github::source::fetch_source(
                    repo,
                    &repo_path,
                    None,
                    effective_token,
                )
                .await;
                if let Some(src) = content {
                    let _ = db.cache_source(repo, &file_path, commit_ref, &src).await;
                    file.source_content = Some(src);
                }
            }
        }
    }

    let page = FileCoveragePage {
        project,
        file_path,
        file,
        line_delta: None,
        branch_delta: None,
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

async fn find_file_with_delta(
    db: &Database,
    project_id: &str,
    file_path: &str,
) -> Result<(FileCoverage, Option<f64>, Option<f64>), StatusCode> {
    let target_names = db
        .get_targets_for_project(project_id)
        .await
        .unwrap_or_default();

    for tname in &target_names {
        let snaps = db
            .get_snapshots_for_project_by_target(project_id, tname, 2)
            .await
            .unwrap_or_default();

        if let Some(snap) = snaps.first() {
            let files: Vec<FileCoverage> = snap
                .files_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .unwrap_or_default();
            if let Some(f) = files.into_iter().find(|f| f.path == file_path) {
                let (line_delta, branch_delta) = if let Some(prev) = snaps.get(1) {
                    let prev_files: Vec<FileCoverage> = prev
                        .files_json
                        .as_ref()
                        .and_then(|json| serde_json::from_str(json).ok())
                        .unwrap_or_default();
                    if let Some(pf) = prev_files.iter().find(|pf| pf.path == file_path) {
                        (Some(f.line_rate - pf.line_rate), Some(f.branch_rate - pf.branch_rate))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };
                return Ok((f, line_delta, branch_delta));
            }
        }
    }

    Err(StatusCode::NOT_FOUND)
}
