use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use omnivore_core::model::coverage::CoverageSnapshot;
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;

use super::{
    compute_composite, fmt_delta_html, fmt_pct_val, html_escape, rate_color_with_threshold,
    CompositeSnapshot, FileTreeNode, HotspotFile, TargetSnapshot, TrendEntry,
};

/// Snapshot option for export modal dropdowns.
pub struct ExportSnapshotOption {
    pub id: String,
    pub commit_sha_short: String,
    pub date_display: String,
}

#[derive(Template)]
#[template(path = "project_detail.html")]
pub struct ProjectDetailPage {
    project: Project,
    latest: Option<CoverageSnapshot>,
    targets: Vec<TargetSnapshot>,
    composite: Option<CompositeSnapshot>,
    has_dependencies: bool,
    export_snapshots: Vec<ExportSnapshotOption>,
    line_threshold: f64,
    branch_threshold: f64,
    line_warn_threshold: f64,
    branch_warn_threshold: f64,
    global_line_threshold: f64,
    global_branch_threshold: f64,
    global_line_warn_threshold: f64,
    global_branch_warn_threshold: f64,
}

impl ProjectDetailPage {
    fn fmt_pct(&self, rate: &f64) -> String {
        fmt_pct_val(*rate)
    }
    fn rate_color(&self, rate: &f64) -> &'static str {
        rate_color_with_threshold(*rate, self.line_threshold, self.line_warn_threshold)
    }
    fn project_line_pct(&self) -> String {
        self.project.line_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_branch_pct(&self) -> String {
        self.project.branch_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_line_warn_pct(&self) -> String {
        self.project.line_warn_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn project_branch_warn_pct(&self) -> String {
        self.project.branch_warn_threshold.map(|v| format!("{:.0}", v * 100.0)).unwrap_or_default()
    }
    fn global_line_pct(&self) -> String {
        format!("{:.0}", self.global_line_threshold * 100.0)
    }
    fn global_branch_pct(&self) -> String {
        format!("{:.0}", self.global_branch_threshold * 100.0)
    }
    fn global_line_warn_pct(&self) -> String {
        format!("{:.0}", self.global_line_warn_threshold * 100.0)
    }
    fn global_branch_warn_pct(&self) -> String {
        format!("{:.0}", self.global_branch_warn_threshold * 100.0)
    }
    fn fmt_delta(&self, delta: &Option<f64>) -> String {
        fmt_delta_html(*delta)
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
    fn hotspots(&self) -> Vec<HotspotFile> {
        let mut all: Vec<HotspotFile> = Vec::new();
        for t in &self.targets {
            for f in &t.files {
                let total = f.lines.len() as i64;
                let covered = f.lines.iter().filter(|l| l.hit_count > 0).count() as i64;
                let uncovered = total - covered;
                if uncovered > 0 {
                    all.push(HotspotFile {
                        path: f.path.clone(),
                        line_rate: f.line_rate,
                        uncovered_lines: uncovered,
                        total_lines: total,
                    });
                }
            }
        }
        all.sort_by(|a, b| b.uncovered_lines.cmp(&a.uncovered_lines)
            .then(a.line_rate.partial_cmp(&b.line_rate).unwrap_or(std::cmp::Ordering::Equal)));
        all.truncate(15);
        all
    }
    fn has_hotspots(&self) -> bool {
        self.targets.iter().any(|t| t.files.iter().any(|f| {
            let covered = f.lines.iter().filter(|l| l.hit_count > 0).count();
            covered < f.lines.len()
        }))
    }

    fn render_file_tree(&self, nodes: &[FileTreeNode], depth: usize) -> String {
        let mut html = String::new();
        self.render_file_tree_inner(nodes, depth, "", &mut html);
        html
    }

    fn render_file_tree_inner(&self, nodes: &[FileTreeNode], depth: usize, parent_id: &str, html: &mut String) {
        for node in nodes {
            let line_color = rate_color_with_threshold(node.line_rate, self.line_threshold, self.line_warn_threshold);
            let branch_color = rate_color_with_threshold(node.branch_rate, self.branch_threshold, self.branch_warn_threshold);
            let indent_px = depth * 20 + 12;
            let delta_html = fmt_delta_html(node.line_delta);
            let branch_delta_html = fmt_delta_html(node.branch_delta);

            if node.is_dir {
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
                    r#"<td><span style="font-weight:600;color:{}">{}</span> {}</td>"#,
                    line_color, fmt_pct_val(node.line_rate), delta_html,
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span> {}</td>"#,
                    branch_color, fmt_pct_val(node.branch_rate), branch_delta_html,
                ));
                html.push_str(&format!(
                    r#"<td><div class="coverage-bar"><div class="coverage-bar-fill" style="width:{}%;--rate:{:.4};"></div></div></td>"#,
                    fmt_pct_val(node.line_rate), node.line_rate,
                ));
                html.push_str("</tr>");
                self.render_file_tree_inner(&node.children, depth + 1, &dir_id, html);
            } else {
                let hidden = if depth == 0 { "" } else { " style=\"display:none;\"" };
                html.push_str(&format!(
                    r#"<tr class="tree-file" data-depth="{depth}" data-parent="{parent_id}" data-path="{path}" data-line-rate="{lr:.6}" data-branch-rate="{br:.6}"{hidden}>"#,
                    path = html_escape(&node.full_path),
                    lr = node.line_rate,
                    br = node.branch_rate,
                ));
                html.push_str(&format!(
                    r#"<td style="padding-left:{}px;"><a href="/projects/{}/files/{}" class="file-path">{}</a></td>"#,
                    indent_px,
                    html_escape(&self.project.id),
                    html_escape(&node.full_path),
                    html_escape(&node.name),
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span> {}</td>"#,
                    line_color, fmt_pct_val(node.line_rate), delta_html,
                ));
                html.push_str(&format!(
                    r#"<td><span style="font-weight:600;color:{}">{}</span> {}</td>"#,
                    branch_color, fmt_pct_val(node.branch_rate), branch_delta_html,
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

    let target_names = db
        .get_targets_for_project(&project_id)
        .await
        .unwrap_or_default();

    let mut targets = Vec::new();
    for tname in &target_names {
        let snaps = db
            .get_snapshots_for_project_by_target(&project_id, tname, 30)
            .await
            .unwrap_or_default();
        if let Some(snap) = snaps.first() {
            let prev = snaps.get(1);
            let mut trend: Vec<TrendEntry> = snaps
                .iter()
                .map(|s| TrendEntry {
                    line_rate: s.line_rate,
                    branch_rate: s.branch_rate,
                    created_at: s.created_at.to_rfc3339(),
                })
                .collect();
            trend.reverse();
            targets.push(TargetSnapshot::from_snapshot(snap, prev, trend));
        }
    }

    let composite = compute_composite(&targets);

    let has_dependencies = latest
        .as_ref()
        .and_then(|s| s.dependencies_json.as_ref())
        .is_some();

    // Build deduplicated snapshot list for export modal
    let all_snaps = db
        .get_snapshots_for_project(&project_id, 50)
        .await
        .unwrap_or_default();
    let mut seen_dates = std::collections::HashSet::new();
    let export_snapshots: Vec<ExportSnapshotOption> = all_snaps
        .iter()
        .filter_map(|s| {
            let date_key = s.created_at.format("%Y-%m-%d %H:%M").to_string();
            if !seen_dates.insert(date_key) {
                return None;
            }
            let sha_short = s
                .commit_sha
                .as_deref()
                .filter(|sha| !sha.is_empty())
                .map(|sha| if sha.len() > 7 { &sha[..7] } else { sha })
                .unwrap_or("")
                .to_string();
            Some(ExportSnapshotOption {
                id: s.id.clone(),
                commit_sha_short: sha_short,
                date_display: s.created_at.format("%b %d, %Y %H:%M").to_string(),
            })
        })
        .collect();

    // Resolve effective thresholds
    let global_settings = db.get_global_settings().await.unwrap_or_default();
    let line_threshold = project
        .line_threshold
        .unwrap_or(global_settings.default_line_threshold);
    let branch_threshold = project
        .branch_threshold
        .unwrap_or(global_settings.default_branch_threshold);
    let line_warn_threshold = project
        .line_warn_threshold
        .unwrap_or(global_settings.default_line_warn_threshold);
    let branch_warn_threshold = project
        .branch_warn_threshold
        .unwrap_or(global_settings.default_branch_warn_threshold);

    let page = ProjectDetailPage {
        project,
        latest,
        targets,
        composite,
        has_dependencies,
        export_snapshots,
        line_threshold,
        branch_threshold,
        line_warn_threshold,
        branch_warn_threshold,
        global_line_threshold: global_settings.default_line_threshold,
        global_branch_threshold: global_settings.default_branch_threshold,
        global_line_warn_threshold: global_settings.default_line_warn_threshold,
        global_branch_warn_threshold: global_settings.default_branch_warn_threshold,
    };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}
