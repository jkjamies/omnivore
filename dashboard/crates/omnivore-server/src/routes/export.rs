use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use omnivore_core::storage::Database;
use serde::Deserialize;

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
        if !current_id.is_empty() {
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
    }
    if current_snapshots.is_empty() {
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
                let snap = snap.filter(|s| current_snapshots.iter().all(|c| c.id != s.id));
                baseline_snapshots.push(snap);
            }
        }
    }

    if baseline_snapshots.is_empty() {
        baseline_snapshots = current_snapshots.iter().map(|_| None).collect();
    }

    let report = omnivore_core::report::build_export_report(
        &project,
        &current_snapshots,
        &baseline_snapshots,
    );

    // project_id is interpolated into a Content-Disposition filename. A quote
    // or control character there produces a malformed header (axum then drops
    // the response entirely), so reduce it to characters that are safe in a
    // filename.
    let safe_id = sanitize_filename(&project_id);

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
                            safe_id
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
                            safe_id
                        ),
                    ),
                ],
                body,
            )
                .into_response())
        }
    }
}

/// Reduce a project ID to characters safe inside a quoted `filename=` value.
///
/// Project IDs come from uploaded reports, so they can contain quotes, spaces,
/// or control characters — all of which corrupt the header.
fn sanitize_filename(id: &str) -> String {
    let cleaned: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .take(100)
        .collect();
    if cleaned.trim_matches('-').is_empty() {
        "project".to_string()
    } else {
        cleaned
    }
}
