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
    assert_eq!(resp["project_id"], "omnivore-test-rig");
    assert_eq!(resp["format"], "Omnivore");

    // Verify project was created
    let req = Request::get("/api/v1/projects")
        .body(Body::empty())
        .unwrap();
    let (_, body) = send(db.clone(), req).await;
    let projects = json_body(&body);
    assert_eq!(projects[0]["id"], "omnivore-test-rig");

    // Verify latest snapshot
    let req = Request::get("/api/v1/coverage/omnivore-test-rig/latest")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db.clone(), req).await;
    assert_eq!(status, 200);

    let snapshot = json_body(&body);
    assert_eq!(snapshot["project_id"], "omnivore-test-rig");
    assert!(snapshot["line_rate"].as_f64().unwrap() > 0.70);
    assert!(snapshot["lines_total"].as_i64().unwrap() > 100);
    assert!(snapshot["files_json"].is_string());

    // Verify trend
    let req = Request::get("/api/v1/coverage/omnivore-test-rig/trend")
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
    assert!(html.contains("omnivore-test-rig"));

    let req = Request::get("/projects/omnivore-test-rig")
        .body(Body::empty())
        .unwrap();
    let (status, body) = send(db, req).await;
    assert_eq!(status, 200);
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("omnivore-test-rig"));
}
