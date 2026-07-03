use crate::model::coverage::{CoverageSnapshot, OmnivoreReport};
use crate::parsers::ParseError;

/// Parse an Omnivore JSON report and convert it to a storable CoverageSnapshot.
///
/// Provenance comes from the report's own `project.source` (falling back to the
/// Omnivore agent), so native reports keep their declared source.
pub fn parse(json: &str) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let report: OmnivoreReport = serde_json::from_str(json)?;
    let snapshot = CoverageSnapshot::from_report(&report, None);
    Ok((report, snapshot))
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

    #[test]
    fn empty_source_falls_back_to_agent() {
        // A wire report with an explicit empty "source" must not persist "" —
        // it falls back to the Omnivore agent.
        let json = r#"{
            "version": "0.1.0", "format": "omnivore",
            "project": {"id": "p", "name": "P", "target": "JVM_UNIT", "source": ""},
            "coverage": {"lineRate": 1.0, "branchRate": 1.0, "linesCovered": 1, "linesTotal": 1, "branchesCovered": 0, "branchesTotal": 0},
            "files": []
        }"#;
        let (_, snapshot) = parse(json).unwrap();
        assert_eq!(snapshot.source, "omnivore-agent");
    }
}
