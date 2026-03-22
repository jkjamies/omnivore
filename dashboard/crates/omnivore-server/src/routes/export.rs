use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use omnivore_core::model::project::Project;
use omnivore_core::storage::Database;
use serde::Deserialize;

// -- Export page --

/// A deduplicated snapshot option for the picker dropdowns (one per point in time).
pub struct SnapshotOption {
    pub id: String,
    pub commit_sha_short: String,
    pub date_display: String,
}

#[derive(Template)]
#[template(path = "export.html")]
pub struct ExportPage {
    project: Project,
    snapshots: Vec<SnapshotOption>,
}

pub async fn export_page(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let all_snaps = db
        .get_snapshots_for_project(&project_id, 50)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Deduplicate by date (minute granularity) so each point in time appears once
    let mut seen_dates = std::collections::HashSet::new();
    let snapshots: Vec<SnapshotOption> = all_snaps
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
            Some(SnapshotOption {
                id: s.id.clone(),
                commit_sha_short: sha_short,
                date_display: s.created_at.format("%b %d, %Y %H:%M").to_string(),
            })
        })
        .collect();

    let page = ExportPage { project, snapshots };
    let html = page.render().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// -- Report download --

#[derive(Deserialize)]
pub struct ReportQuery {
    format: Option<String>,
    current: Option<String>,
    baseline: Option<String>,
}

pub async fn export_report(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Query(params): Query<ReportQuery>,
) -> Result<Response, StatusCode> {
    let project = db
        .get_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let target_names = db
        .get_targets_for_project(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get "current" snapshots — either from the user-picked snapshot or latest per target
    let mut current_snapshots = Vec::new();
    if let Some(current_id) = &params.current {
        if let Ok(Some(picked)) = db.get_snapshot_by_id(current_id).await {
            let picked_date = picked.created_at.to_rfc3339();
            for tname in &target_names {
                if let Ok(Some(snap)) = db
                    .get_snapshot_closest_to_date(&project_id, tname, &picked_date)
                    .await
                {
                    current_snapshots.push(snap);
                }
            }
        }
    }
    if current_snapshots.is_empty() {
        // Fallback: latest per target
        for tname in &target_names {
            if let Ok(Some(snap)) = db.get_latest_snapshot_by_target(&project_id, tname).await {
                current_snapshots.push(snap);
            }
        }
    }

    if current_snapshots.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Get baseline snapshots per target
    let mut baseline_snapshots: Vec<Option<_>> = Vec::new();
    let baseline_id = params.baseline.as_deref().filter(|s| !s.is_empty());

    if let Some(baseline_id) = baseline_id {
        if let Ok(Some(baseline_snap)) = db.get_snapshot_by_id(baseline_id).await {
            let baseline_date = baseline_snap.created_at.to_rfc3339();
            for tname in &target_names {
                let snap = db
                    .get_snapshot_closest_to_date(&project_id, tname, &baseline_date)
                    .await
                    .ok()
                    .flatten();
                // Don't use the same snapshot as both current and baseline
                let snap = snap.filter(|s| current_snapshots.iter().all(|c| c.id != s.id));
                baseline_snapshots.push(snap);
            }
        }
    }

    // If no baselines resolved, pass empty options (report will omit comparison)
    if baseline_snapshots.is_empty() {
        baseline_snapshots = current_snapshots.iter().map(|_| None).collect();
    }

    let report = omnivore_core::report::build_export_report(
        &project,
        &current_snapshots,
        &baseline_snapshots,
    );

    let format = params.format.as_deref().unwrap_or("md");
    match format {
        "json" => {
            let body = omnivore_core::report::render_json(&report);
            Ok((
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (
                        header::CONTENT_DISPOSITION,
                        &format!(
                            "attachment; filename=\"omnivore-report-{}.json\"",
                            project_id
                        ),
                    ),
                ],
                body,
            )
                .into_response())
        }
        _ => {
            let body = omnivore_core::report::render_markdown(&report);
            Ok((
                [
                    (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
                    (
                        header::CONTENT_DISPOSITION,
                        &format!(
                            "attachment; filename=\"omnivore-report-{}.md\"",
                            project_id
                        ),
                    ),
                ],
                body,
            )
                .into_response())
        }
    }
}

fn target_label(target: &str) -> &str {
    match target {
        "JVM_UNIT" | "JvmUnit" => "Unit Tests",
        "ANDROID_INSTRUMENTED" | "AndroidInstrumented" => "Instrumented Tests",
        "IOS_UNIT" | "IosUnit" => "iOS Unit Tests",
        "KOTLIN_NATIVE" | "KotlinNative" => "Kotlin/Native Tests",
        "COMPOSITE" | "Composite" => "Composite",
        other => other,
    }
}
