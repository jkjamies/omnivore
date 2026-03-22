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
