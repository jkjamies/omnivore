use crate::model::coverage::{CoverageSnapshot, OmnivoreReport};
use crate::parsers::ParseError;
use chrono::Utc;
use uuid::Uuid;

/// Parse an Omnivore JSON report and convert it to a storable CoverageSnapshot.
pub fn parse(json: &str) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let report: OmnivoreReport = serde_json::from_str(json)?;
    let snapshot = to_snapshot(&report);
    Ok((report, snapshot))
}

fn to_snapshot(report: &OmnivoreReport) -> CoverageSnapshot {
    let files_json = serde_json::to_string(&report.files).ok();
    let dependencies_json = report
        .dependencies
        .as_ref()
        .and_then(|d| serde_json::to_string(d).ok());

    CoverageSnapshot {
        id: Uuid::new_v4().to_string(),
        project_id: report.project.id.clone(),
        commit_sha: report.project.commit_sha.clone(),
        branch: report.project.branch.clone(),
        target: format!("{:?}", report.project.target),
        line_rate: report.coverage.line_rate,
        branch_rate: report.coverage.branch_rate,
        lines_covered: report.coverage.lines_covered,
        lines_total: report.coverage.lines_total,
        branches_covered: report.coverage.branches_covered,
        branches_total: report.coverage.branches_total,
        file_count: report.files.len() as i64,
        created_at: Utc::now(),
        files_json,
        dependencies_json,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_REPORT: &str = r#"{
        "version": "0.1.0",
        "format": "omnivore",
        "project": {
            "id": "test-project",
            "name": "Test Project",
            "commitSha": "abc123",
            "branch": "main",
            "target": "JVM_UNIT"
        },
        "coverage": {
            "lineRate": 0.78,
            "branchRate": 0.71,
            "linesCovered": 106,
            "linesTotal": 136,
            "branchesCovered": 34,
            "branchesTotal": 48
        },
        "files": [
            {
                "path": "com/example/Foo.kt",
                "lineRate": 0.86,
                "branchRate": 0.89,
                "lines": [
                    {"lineNumber": 10, "hitCount": 1},
                    {"lineNumber": 12, "hitCount": 0}
                ]
            }
        ]
    }"#;

    #[test]
    fn parse_omnivore_json() {
        let (report, snapshot) = parse(SAMPLE_REPORT).unwrap();
        assert_eq!(report.project.id, "test-project");
        assert_eq!(report.project.name, "Test Project");
        assert_eq!(report.coverage.lines_covered, 106);
        assert_eq!(report.files.len(), 1);

        assert_eq!(snapshot.project_id, "test-project");
        assert_eq!(snapshot.line_rate, 0.78);
        assert_eq!(snapshot.file_count, 1);
        assert!(snapshot.commit_sha.as_deref() == Some("abc123"));
    }
}
