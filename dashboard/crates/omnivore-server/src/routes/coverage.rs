use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use omnivore_core::github;
use omnivore_core::model::coverage::{CoverageSnapshot, CoverageTarget, DependencyGraph};
use omnivore_core::parsers::{
    go_coverprofile, jacoco_xml, lcov, llvm_cov, omnivore_json, python_coverage, CoverageFormat,
    IngestMeta,
};
use omnivore_core::storage::Database;
use serde::{Deserialize, Serialize};

/// Query parameters for the universal ingest endpoint.
#[derive(Deserialize, Default)]
pub struct IngestParams {
    /// Explicit format: "omnivore", "lcov", "llvm-cov", "go", "python", "kover",
    /// or "jacoco". Auto-detected from the body if omitted.
    pub format: Option<String>,
    /// Coverage target override (e.g. "JVM_UNIT", "ANDROID_INSTRUMENTED"). Only
    /// used by formats that don't encode their own target (JaCoCo/Kover XML);
    /// defaults to JVM_UNIT there. Ignored by formats with a fixed target.
    pub target: Option<String>,
    /// Project ID (required for lcov/llvm-cov, ignored for omnivore).
    pub project_id: Option<String>,
    /// Project name (required for lcov/llvm-cov, ignored for omnivore).
    pub project_name: Option<String>,
    /// Commit SHA (optional, for lcov/llvm-cov).
    pub commit_sha: Option<String>,
    /// Branch name (optional, for lcov/llvm-cov).
    pub branch: Option<String>,
    /// GitHub repo slug for PR comments (e.g., "owner/repo").
    pub github_repo: Option<String>,
    /// PR number to comment on.
    pub pr_number: Option<u64>,
    /// Base branch to compare against (default: "main").
    pub base_branch: Option<String>,
}

/// Ingest a coverage report. Supports omnivore JSON, lcov, and llvm-cov export JSON.
///
/// Format is auto-detected from content, or can be specified via `?format=` query param.
/// For lcov and llvm-cov formats, project metadata should be provided via query params:
/// `?format=lcov&project_id=my-app&project_name=My+App&commit_sha=abc123&branch=main`
///
/// PR comments: pass `github_repo`, `pr_number`, and optionally `base_branch` as query params.
/// The GitHub token can be provided via `X-GitHub-Token` header or the server's `GITHUB_TOKEN` env var.
pub async fn ingest_coverage(
    State(db): State<Database>,
    headers: axum::http::HeaderMap,
    Query(params): Query<IngestParams>,
    body: String,
) -> Result<(StatusCode, Json<IngestResponse>), (StatusCode, String)> {
    // API key authentication (backwards-compatible: skip if no keys exist)
    let has_keys = db.any_api_keys_exist().await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
    })?;

    let validated_key = if has_keys {
        let raw_key = headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                (StatusCode::UNAUTHORIZED, "Missing X-API-Key header".to_string())
            })?;

        let api_key = db.validate_api_key(raw_key).await.map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {e}"))
        })?;

        Some(api_key.ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Invalid API key".to_string())
        })?)
    } else {
        None
    };

    let format = match &params.format {
        Some(f) => CoverageFormat::from_str_loose(f)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("Unknown format: {f}. Use omnivore, lcov, llvm-cov, go, python, kover, or jacoco")))?,
        None => CoverageFormat::detect(&body)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Could not detect format. Specify ?format= query parameter".into()))?,
    };

    // Every format that lacks embedded project info reads it from the same query
    // params, so build the shared metadata once.
    let meta = IngestMeta {
        project_id: params.project_id.clone(),
        project_name: params.project_name.clone(),
        commit_sha: params.commit_sha.clone(),
        branch: params.branch.clone(),
    };

    let (report, snapshot) = match format {
        CoverageFormat::Omnivore => {
            omnivore_json::parse(&body)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid omnivore JSON: {e}")))?
        }
        CoverageFormat::Lcov => {
            lcov::parse(&body, &meta)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid lcov: {e}")))?
        }
        CoverageFormat::LlvmCov => {
            llvm_cov::parse(&body, &meta)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid llvm-cov export: {e}")))?
        }
        CoverageFormat::GoCoverprofile => {
            go_coverprofile::parse(&body, &meta)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid Go coverprofile: {e}")))?
        }
        CoverageFormat::PythonCoverage => {
            python_coverage::parse(&body, &meta)
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid Python coverage.py JSON: {e}")))?
        }
        CoverageFormat::Jacoco | CoverageFormat::Kover => {
            // JaCoCo XML doesn't encode its execution environment; default to
            // JVM unit coverage, overridable via ?target=. Provenance comes from
            // the format alias (kover vs jacoco).
            let target = match &params.target {
                Some(t) => CoverageTarget::from_str_loose(t).ok_or_else(|| {
                    (StatusCode::BAD_REQUEST, format!("Unknown target: {t}"))
                })?,
                None => CoverageTarget::JvmUnit,
            };
            jacoco_xml::parse(&body, &meta, target, format.source())
                .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid JaCoCo/Kover XML: {e}")))?
        }
    };

    // Project-scoped key: verify it matches the project being uploaded to
    if let Some(ref key) = validated_key {
        if let Some(ref key_project_id) = key.project_id {
            if key_project_id != &snapshot.project_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    format!(
                        "API key is scoped to project '{}', cannot upload to '{}'",
                        key_project_id, snapshot.project_id
                    ),
                ));
            }
        }
    }

    let project_name = report.project.name.clone();
    let ratchet = db.ingest_snapshot(&snapshot, Some(&project_name))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Storage error: {e}"),
            )
        })?;

    let mut warnings = Vec::new();
    if ratchet.line_floor_violated {
        if let Some(floor) = ratchet.line_floor {
            warnings.push(format!(
                "Line coverage {:.1}% is below ratchet floor {:.1}%",
                snapshot.line_rate * 100.0, floor * 100.0
            ));
        }
    }
    if ratchet.branch_floor_violated {
        if let Some(floor) = ratchet.branch_floor {
            warnings.push(format!(
                "Branch coverage {:.1}% is below ratchet floor {:.1}%",
                snapshot.branch_rate * 100.0, floor * 100.0
            ));
        }
    }

    // Post PR comment if GitHub params are provided
    if let (Some(repo), Some(pr_number)) = (&params.github_repo, params.pr_number) {
        // Token from header takes priority, then server env var
        let github_token = headers
            .get("X-GitHub-Token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .or_else(|| std::env::var("GITHUB_TOKEN").ok());

        if let Some(token) = github_token {
            let base_branch = params.base_branch.as_deref().unwrap_or("main");
            let baseline = db
                .get_latest_snapshot_for_branch(&snapshot.project_id, base_branch)
                .await
                .unwrap_or(None);

            let dashboard_url = std::env::var("OMNIVORE_DASHBOARD_URL").ok();
            let comment_body = github::generate_comment(
                &snapshot,
                baseline.as_ref(),
                dashboard_url.as_deref(),
            );

            let client = github::GitHubClient::new(token, None);
            if let Err(e) = client.post_or_update_comment(repo, pr_number, &comment_body).await {
                tracing::warn!("Failed to post PR comment to {repo}#{pr_number}: {e}");
            }
        } else {
            tracing::warn!("PR comment requested but GITHUB_TOKEN not set");
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(IngestResponse {
            id: snapshot.id,
            project_id: snapshot.project_id,
            format: format!("{format:?}"),
            line_rate: snapshot.line_rate,
            branch_rate: snapshot.branch_rate,
            warnings,
        }),
    ))
}

#[derive(Serialize)]
pub struct IngestResponse {
    pub id: String,
    pub project_id: String,
    pub format: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// Get the latest coverage snapshot for a project.
pub async fn get_latest(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Json<CoverageSnapshot>, StatusCode> {
    db.get_latest_snapshot(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Get coverage trend for a project.
pub async fn get_trend(
    State(db): State<Database>,
    Path(project_id): Path<String>,
    Query(params): Query<TrendParams>,
) -> Result<Json<Vec<TrendPoint>>, StatusCode> {
    let limit = params.limit.unwrap_or(30);
    let snapshots = db
        .get_snapshots_for_project(&project_id, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let points: Vec<TrendPoint> = snapshots
        .into_iter()
        .map(|s| TrendPoint {
            commit_sha: s.commit_sha,
            branch: s.branch,
            target: s.target,
            line_rate: s.line_rate,
            branch_rate: s.branch_rate,
            lines_covered: s.lines_covered,
            lines_total: s.lines_total,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(points))
}

#[derive(Deserialize)]
pub struct TrendParams {
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct TrendPoint {
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
    pub target: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub created_at: String,
}

/// Get the dependency graph from the latest snapshot for a project.
pub async fn get_dependencies(
    State(db): State<Database>,
    Path(project_id): Path<String>,
) -> Result<Json<DependencyGraph>, StatusCode> {
    let snapshot = db
        .get_latest_snapshot(&project_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let graph: DependencyGraph = snapshot
        .dependencies_json
        .as_deref()
        .ok_or(StatusCode::NOT_FOUND)
        .and_then(|json| serde_json::from_str(json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR))?;

    Ok(Json(graph))
}
