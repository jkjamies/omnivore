use axum::body::Body;
use http_body_util::BodyExt;
use hyper::Request;
use omnivore_core::storage::Database;
use omnivore_server::build_router;
use serde_json::Value;
use tower::ServiceExt;

/// Create an in-memory database for testing.
async fn test_db() -> Database {
    Database::new("sqlite::memory:").await.unwrap()
}

/// Helper: make a request and return (status, body bytes).
async fn send(
    db: Database,
    req: Request<Body>,
) -> (hyper::StatusCode, Vec<u8>) {
    let app = build_router(db);
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, bytes)
}

fn json_body(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

// ── Health ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let db = test_db().await;
    let req = Request::get("/api/v1/health")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let json = json_body(&body);
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], "0.1.0");
}

// ── Projects ────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_projects_initially_empty() {
    let db = test_db().await;
    let req = Request::get("/api/v1/projects")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);
    assert_eq!(json_body(&body), serde_json::json!([]));
}

#[tokio::test]
async fn create_and_list_project() {
    let db = test_db().await;

    // Create
    let create_req = Request::post("/api/v1/projects")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "id": "test-proj",
                "name": "Test Project",
                "description": "A test project"
            })
            .to_string(),
        ))
        .unwrap();

    let (status, body) = send(db.clone(), create_req).await;
    assert_eq!(status, 201);

    let created = json_body(&body);
    assert_eq!(created["id"], "test-proj");
    assert_eq!(created["name"], "Test Project");
    assert_eq!(created["description"], "A test project");

    // List
    let list_req = Request::get("/api/v1/projects")
        .body(Body::empty())
        .unwrap();

    let (status, body) = send(db, list_req).await;
    assert_eq!(status, 200);

    let projects = json_body(&body);
    assert_eq!(projects.as_array().unwrap().len(), 1);
    assert_eq!(projects[0]["id"], "test-proj");
}

// ── Omnivore JSON Ingestion ─────────────────────────────────────────────

fn sample_omnivore_report() -> String {
    serde_json::json!({
        "version": "0.1.0",
        "format": "omnivore",
        "project": {
            "id": "my-app",
            "name": "My App",
            "commitSha": "abc123",
            "branch": "main",
            "target": "JVM_UNIT"
        },
        "coverage": {
            "lineRate": 0.85,
            "branchRate": 0.70,
            "linesCovered": 85,
            "linesTotal": 100,
            "branchesCovered": 14,
            "branchesTotal": 20
        },
        "files": [
            {
                "path": "src/Main.kt",
                "lineRate": 0.90,
                "branchRate": 0.75,
                "lines": [
                    {"lineNumber": 1, "hitCount": 1},
                    {"lineNumber": 2, "hitCount": 0},
                    {"lineNumber": 3, "hitCount": 1}
                ]
            }
        ]
    })
    .to_string()
}

#[tokio::test]
async fn ingest_omnivore_json() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_omnivore_report()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["project_id"], "my-app");
    assert_eq!(resp["format"], "Omnivore");
    assert_eq!(resp["line_rate"], 0.85);
    assert_eq!(resp["branch_rate"], 0.70);
}

#[tokio::test]
async fn ingest_omnivore_auto_creates_project() {
    let db = test_db().await;

    // Ingest
    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_omnivore_report()))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    // Verify project was auto-created
    let req = Request::get("/api/v1/projects")
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(db, req).await;
    let projects = json_body(&body);
    assert_eq!(projects.as_array().unwrap().len(), 1);
    assert_eq!(projects[0]["id"], "my-app");
    assert_eq!(projects[0]["name"], "My App");
}

// ── lcov Ingestion ──────────────────────────────────────────────────────

fn sample_lcov() -> String {
    "\
TN:test
SF:src/main.go
DA:1,1
DA:2,1
DA:3,0
DA:4,1
BRDA:2,0,0,1
BRDA:2,0,1,0
BRF:2
BRH:1
LF:4
LH:3
end_of_record
"
    .to_string()
}

#[tokio::test]
async fn ingest_lcov_with_explicit_format() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=lcov&project_id=go-svc&project_name=Go+Service")
        .body(Body::from(sample_lcov()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["project_id"], "go-svc");
    assert_eq!(resp["format"], "Lcov");
    assert_eq!(resp["line_rate"], 0.75);
}

#[tokio::test]
async fn ingest_lcov_auto_detected() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?project_id=go-svc")
        .body(Body::from(sample_lcov()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["format"], "Lcov");
}

// ── llvm-cov Ingestion ──────────────────────────────────────────────────

fn sample_llvm_cov() -> String {
    serde_json::json!({
        "type": "llvm.coverage.json.export",
        "version": "2.0.1",
        "data": [
            {
                "files": [
                    {
                        "filename": "src/main.rs",
                        "segments": [
                            [1, 1, 5, true, true],
                            [3, 1, 0, true, true],
                            [5, 1, 2, true, true]
                        ],
                        "summary": {
                            "lines": {"count": 5, "covered": 4, "percent": 80.0},
                            "branches": {"count": 2, "covered": 1, "percent": 50.0}
                        }
                    }
                ],
                "totals": {
                    "lines": {"count": 5, "covered": 4, "percent": 80.0},
                    "branches": {"count": 2, "covered": 1, "percent": 50.0}
                }
            }
        ]
    })
    .to_string()
}

#[tokio::test]
async fn ingest_llvm_cov() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=llvm-cov&project_id=rust-app&project_name=Rust+App")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_llvm_cov()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["project_id"], "rust-app");
    assert_eq!(resp["format"], "LlvmCov");
    assert_eq!(resp["line_rate"], 0.8);
}

#[tokio::test]
async fn ingest_llvm_cov_auto_detected() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?project_id=rust-app")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_llvm_cov()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["format"], "LlvmCov");
}

// ── JaCoCo / Kover XML Ingestion ─────────────────────────────────────────

fn sample_jacoco() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd">
<report name="kmp-test-rig">
  <package name="com/example/app">
    <sourcefile name="Calculator.kt">
      <line nr="5" mi="0" ci="4" mb="0" cb="0"/>
      <line nr="6" mi="0" ci="6" mb="1" cb="1"/>
      <line nr="7" mi="3" ci="0" mb="0" cb="0"/>
    </sourcefile>
  </package>
  <counter type="LINE" missed="1" covered="2"/>
  <counter type="BRANCH" missed="1" covered="1"/>
</report>
"#
    .to_string()
}

#[tokio::test]
async fn ingest_kover_explicit_format() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=kover&project_id=kmp-app&project_name=KMP+App")
        .body(Body::from(sample_jacoco()))
        .unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["project_id"], "kmp-app");
    assert_eq!(resp["format"], "Kover");
    // 2/3 lines covered.
    assert!((resp["line_rate"].as_f64().unwrap() - 2.0 / 3.0).abs() < 1e-6);
}

#[tokio::test]
async fn ingest_jacoco_auto_detected_sets_target_and_source() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?project_id=kmp-app")
        .body(Body::from(sample_jacoco()))
        .unwrap();

    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 201);
    assert_eq!(json_body(&body)["format"], "Jacoco");

    // Auto-detected XML defaults to JVM_UNIT and records jacoco provenance.
    let snap = db.get_latest_snapshot("kmp-app").await.unwrap().unwrap();
    assert_eq!(snap.target, "JVM_UNIT");
    assert_eq!(snap.source, "jacoco");
}

#[tokio::test]
async fn ingest_kover_target_override() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=kover&project_id=kmp-app&target=ANDROID_INSTRUMENTED")
        .body(Body::from(sample_jacoco()))
        .unwrap();

    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    let snap = db.get_latest_snapshot("kmp-app").await.unwrap().unwrap();
    assert_eq!(snap.target, "ANDROID_INSTRUMENTED");
    assert_eq!(snap.source, "kover");
}

#[tokio::test]
async fn ingest_kover_and_agent_form_separate_series() {
    let db = test_db().await;

    // Kover import for JVM_UNIT.
    let req = Request::post("/api/v1/ingest/coverage?format=kover&project_id=dual")
        .body(Body::from(sample_jacoco()))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    // Native agent JVM_UNIT for the same project.
    let omni = serde_json::json!({
        "version": "0.1.0", "format": "omnivore",
        "project": {"id": "dual", "name": "Dual", "target": "JVM_UNIT"},
        "coverage": {"lineRate": 0.9, "branchRate": 0.8, "linesCovered": 9, "linesTotal": 10, "branchesCovered": 8, "branchesTotal": 10},
        "files": [{"path": "A.kt", "lineRate": 0.9, "branchRate": 0.8, "lines": [{"lineNumber": 1, "hitCount": 1}]}]
    }).to_string();
    let req = Request::post("/api/v1/ingest/coverage?format=omnivore")
        .header("Content-Type", "application/json")
        .body(Body::from(omni))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    // Same target, two sources → two distinct series.
    let series = db.get_series_for_project("dual").await.unwrap();
    assert_eq!(series.len(), 2, "expected two (target, source) series, got {series:?}");
    assert!(series.contains(&("JVM_UNIT".to_string(), "kover".to_string())));
    assert!(series.contains(&("JVM_UNIT".to_string(), "omnivore-agent".to_string())));
}

// ── Bad requests ────────────────────────────────────────────────────────

#[tokio::test]
async fn ingest_bad_format_returns_400() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=cobertura")
        .body(Body::from("{}"))
        .unwrap();

    let (status, _) = send(db, req).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn ingest_invalid_json_returns_400() {
    let db = test_db().await;
    let req = Request::post("/api/v1/ingest/coverage?format=omnivore")
        .header("Content-Type", "application/json")
        .body(Body::from("not json at all"))
        .unwrap();

    let (status, _) = send(db, req).await;
    assert_eq!(status, 400);
}

// ── Coverage queries ────────────────────────────────────────────────────

#[tokio::test]
async fn get_latest_returns_404_when_empty() {
    let db = test_db().await;
    let req = Request::get("/api/v1/coverage/nonexistent/latest")
        .body(Body::empty())
        .unwrap();

    let (status, _) = send(db, req).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn get_latest_after_ingest() {
    let db = test_db().await;

    // Ingest
    let ingest = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_omnivore_report()))
        .unwrap();
    let (status, _) = send(db.clone(), ingest).await;
    assert_eq!(status, 201);

    // Query latest
    let req = Request::get("/api/v1/coverage/my-app/latest")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let snapshot = json_body(&body);
    assert_eq!(snapshot["project_id"], "my-app");
    assert_eq!(snapshot["line_rate"], 0.85);
    assert_eq!(snapshot["lines_covered"], 85);
    assert_eq!(snapshot["lines_total"], 100);
}

#[tokio::test]
async fn get_trend_after_ingest() {
    let db = test_db().await;

    // Ingest twice
    for _ in 0..2 {
        let req = Request::post("/api/v1/ingest/coverage")
            .header("Content-Type", "application/json")
            .body(Body::from(sample_omnivore_report()))
            .unwrap();
        let (status, _) = send(db.clone(), req).await;
        assert_eq!(status, 201);
    }

    // Query trend
    let req = Request::get("/api/v1/coverage/my-app/trend?limit=10")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let trend = json_body(&body);
    assert_eq!(trend.as_array().unwrap().len(), 2);
    assert_eq!(trend[0]["line_rate"], 0.85);
}

#[tokio::test]
async fn get_dependencies_returns_404_when_none() {
    let db = test_db().await;

    // Ingest a report without dependencies
    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_omnivore_report()))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    let req = Request::get("/api/v1/coverage/my-app/dependencies")
        .body(Body::empty())
        .unwrap();
    let (status, _) = send(db, req).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn get_dependencies_with_graph() {
    let db = test_db().await;

    // Ingest a report with dependencies
    let mut report: Value = serde_json::from_str(&sample_omnivore_report()).unwrap();
    report["dependencies"] = serde_json::json!({
        "modules": [
            {"id": "app", "name": "app", "type": "INTERNAL"},
            {"id": "lib", "name": "lib", "type": "INTERNAL"}
        ],
        "edges": [
            {"from": "app", "to": "lib", "configuration": "implementation"}
        ]
    });

    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(report.to_string()))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    // Query dependencies
    let req = Request::get("/api/v1/coverage/my-app/dependencies")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let graph = json_body(&body);
    assert_eq!(graph["modules"].as_array().unwrap().len(), 2);
    assert_eq!(graph["edges"].as_array().unwrap().len(), 1);
    assert_eq!(graph["edges"][0]["from"], "app");
}

// ── HTML pages ──────────────────────────────────────────────────────────

#[tokio::test]
async fn projects_page_renders_html() {
    let db = test_db().await;
    let req = Request::get("/").body(Body::empty()).unwrap();

    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("<!doctype html>") || html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Omnivore"));
}

#[tokio::test]
async fn project_detail_page_after_ingest() {
    let db = test_db().await;

    // Ingest first
    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(sample_omnivore_report()))
        .unwrap();
    let (status, _) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    // Render detail page
    let req = Request::get("/projects/my-app")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);

    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("My App") || html.contains("my-app"));
}

// ── End-to-end: ingest test-rig report ──────────────────────────────────

#[tokio::test]
async fn end_to_end_test_rig_report() {
    let report = include_str!("../../../../test-rigs/kmp-test-rig/build/reports/omnivore/omnivore-report.json");

    let db = test_db().await;

    // Ingest the real test-rig report
    let req = Request::post("/api/v1/ingest/coverage")
        .header("Content-Type", "application/json")
        .body(Body::from(report.to_string()))
        .unwrap();
    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 201);

    let resp = json_body(&body);
    assert_eq!(resp["project_id"], "kmp-test-rig");
    assert_eq!(resp["format"], "Omnivore");

    // Verify project was created
    let req = Request::get("/api/v1/projects")
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(db.clone(), req).await;
    let projects = json_body(&body);
    assert_eq!(projects[0]["id"], "kmp-test-rig");

    // Verify latest snapshot
    let req = Request::get("/api/v1/coverage/kmp-test-rig/latest")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 200);

    let snapshot = json_body(&body);
    assert_eq!(snapshot["project_id"], "kmp-test-rig");
    assert!(snapshot["line_rate"].as_f64().unwrap() > 0.50);
    assert!(snapshot["lines_total"].as_i64().unwrap() > 50);
    assert!(snapshot["files_json"].is_string());

    // Verify trend
    let req = Request::get("/api/v1/coverage/kmp-test-rig/trend")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 200);
    let trend = json_body(&body);
    assert_eq!(trend.as_array().unwrap().len(), 1);

    // Verify HTML pages render
    let req = Request::get("/").body(Body::empty()).unwrap();
    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 200);
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("kmp-test-rig"));

    let req = Request::get("/projects/kmp-test-rig")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("kmp-test-rig"));
}

// ── Retention pruning ──────────────────────────────────────────────────

#[tokio::test]
async fn retention_prunes_old_snapshots() {
    use omnivore_core::model::coverage::CoverageSnapshot;
    use chrono::Utc;

    let db = test_db().await;

    // Set low retention limits for testing via DB
    use omnivore_core::model::settings::GlobalSettings;
    let mut settings = db.get_global_settings().await.unwrap();
    settings.retention_full = 3;
    settings.retention_summary = 2;
    db.update_global_settings(&settings).await.unwrap();

    // Create project
    db.create_project(&omnivore_core::model::project::CreateProject {
        id: "retention-test".to_string(),
        name: "Retention Test".to_string(),
        description: None,
        github_repo: None,
        source_root: None,
        line_threshold: None,
        branch_threshold: None,
        line_warn_threshold: None,
        branch_warn_threshold: None,
    }).await.unwrap();

    // Insert 7 snapshots (exceeds full=3, summary=2, total=5)
    for i in 0..7 {
        let snap = CoverageSnapshot {
            id: format!("snap-{i}"),
            project_id: "retention-test".to_string(),
            commit_sha: Some(format!("abc{i}")),
            branch: Some("main".to_string()),
            target: "JVM_UNIT".to_string(),
            source: "omnivore-agent".to_string(),
            line_rate: 0.5 + (i as f64 * 0.05),
            branch_rate: 0.4,
            lines_covered: 50 + i,
            lines_total: 100,
            branches_covered: 40,
            branches_total: 100,
            file_count: 10,
            created_at: Utc::now(),
            files_json: Some(format!(r#"[{{"path":"file{i}.kt","lineRate":0.5}}]"#)),
            dependencies_json: None,
        };
        db.insert_snapshot(&snap).await.unwrap();
        db.prune_snapshots("retention-test", "JVM_UNIT", "omnivore-agent").await.unwrap();
        // Small delay to ensure ordering by created_at
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // Query all remaining snapshots
    let all = db.get_snapshots_for_project("retention-test", 100).await.unwrap();

    // Should have 5 total (3 full + 2 summary), 2 deleted
    assert_eq!(all.len(), 5, "Expected 5 snapshots after pruning, got {}", all.len());

    // The 3 newest should have files_json
    let with_files: Vec<_> = all.iter().filter(|s| s.files_json.is_some()).collect();
    assert_eq!(with_files.len(), 3, "Expected 3 snapshots with full file data, got {}", with_files.len());

    // The 2 oldest remaining should have files_json = None (summary-only)
    let without_files: Vec<_> = all.iter().filter(|s| s.files_json.is_none()).collect();
    assert_eq!(without_files.len(), 2, "Expected 2 summary-only snapshots, got {}", without_files.len());

    // Clean up env vars
    // SAFETY: test runs single-threaded for this env manipulation
    unsafe {
        std::env::remove_var("OMNIVORE_RETENTION_FULL");
        std::env::remove_var("OMNIVORE_RETENTION_SUMMARY");
    }
}
