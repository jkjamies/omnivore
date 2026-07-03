use crate::model::coverage::{
    source, CoverageSummary, CoverageSnapshot, CoverageTarget, FileCoverage, LineCoverage,
    OmnivoreReport, ProjectInfo,
};
use crate::parsers::{IngestMeta, ParseError};
use serde::Deserialize;

/// Metadata not present in coverage.py JSON — must be supplied externally.
pub type PythonCoverageMeta = IngestMeta;

/// Top-level coverage.py JSON structure.
#[derive(Deserialize)]
struct CoverageJson {
    meta: CoverageMeta,
    files: std::collections::HashMap<String, FileEntry>,
    totals: TotalsSummary,
}

#[derive(Deserialize)]
struct CoverageMeta {
    #[serde(default)]
    branch_coverage: bool,
}

#[derive(Deserialize)]
struct FileEntry {
    executed_lines: Vec<i32>,
    missing_lines: Vec<i32>,
    #[serde(default)]
    summary: FileSummary,
}

#[derive(Deserialize, Default)]
struct FileSummary {
    #[serde(default)]
    num_branches: Option<i64>,
    #[serde(default)]
    covered_branches: Option<i64>,
    #[serde(default)]
    #[allow(dead_code)]
    missing_branches: Option<i64>,
}

#[derive(Deserialize)]
struct TotalsSummary {
    covered_lines: i64,
    num_statements: i64,
    #[serde(default)]
    num_branches: Option<i64>,
    #[serde(default)]
    covered_branches: Option<i64>,
}

/// Parse a coverage.py JSON report (`coverage json` output).
pub fn parse(input: &str, meta: &PythonCoverageMeta) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let cov: CoverageJson = serde_json::from_str(input)
        .map_err(|e| ParseError::PythonCoverage(format!("Invalid JSON: {e}")))?;

    if cov.files.is_empty() {
        return Err(ParseError::PythonCoverage("No files in coverage report".into()));
    }

    let mut file_coverages: Vec<FileCoverage> = Vec::new();
    let mut sorted_files: Vec<String> = cov.files.keys().cloned().collect();
    sorted_files.sort();

    for file_name in &sorted_files {
        let entry = &cov.files[file_name];

        // Build per-line coverage: executed lines get count=1, missing get count=0
        let mut lines: Vec<LineCoverage> = Vec::new();
        for &ln in &entry.executed_lines {
            lines.push(LineCoverage { line_number: ln, hit_count: 1 });
        }
        for &ln in &entry.missing_lines {
            lines.push(LineCoverage { line_number: ln, hit_count: 0 });
        }
        lines.sort_by_key(|l| l.line_number);

        let total = lines.len() as f64;
        let hit = entry.executed_lines.len() as f64;
        let line_rate = if total > 0.0 { hit / total } else { 0.0 };

        let branch_rate = if cov.meta.branch_coverage {
            let num_b = entry.summary.num_branches.unwrap_or(0);
            let cov_b = entry.summary.covered_branches.unwrap_or(0);
            if num_b > 0 { cov_b as f64 / num_b as f64 } else { 0.0 }
        } else {
            0.0
        };

        file_coverages.push(FileCoverage {
            path: file_name.clone(),
            line_rate,
            branch_rate,
            lines,
            source_content: None,
        });
    }

    let lines_total = cov.totals.num_statements;
    let lines_covered = cov.totals.covered_lines;
    let line_rate = if lines_total > 0 {
        lines_covered as f64 / lines_total as f64
    } else {
        0.0
    };

    let branches_total = cov.totals.num_branches.unwrap_or(0);
    let branches_covered = cov.totals.covered_branches.unwrap_or(0);
    let branch_rate = if branches_total > 0 {
        branches_covered as f64 / branches_total as f64
    } else {
        0.0
    };

    let project_id = meta.project_id.clone().unwrap_or_else(|| "python-project".into());
    let project_name = meta.project_name.clone().unwrap_or_else(|| "python import".into());

    let report = OmnivoreReport {
        version: "0.1.0".into(),
        format: "python-coverage".into(),
        dependencies: None,
        project: ProjectInfo {
            id: project_id,
            name: project_name,
            commit_sha: meta.commit_sha.clone(),
            branch: meta.branch.clone(),
            target: CoverageTarget::PythonCoverage,
            source: Some(source::PYTHON_COVERAGE.into()),
        },
        coverage: CoverageSummary {
            line_rate,
            branch_rate,
            lines_covered,
            lines_total,
            branches_covered,
            branches_total,
        },
        files: file_coverages,
    };

    let snapshot = CoverageSnapshot::from_report(&report, Some(source::PYTHON_COVERAGE));
    Ok((report, snapshot))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_COVERAGE: &str = r#"{
        "meta": {
            "format": 3,
            "version": "7.13.5",
            "timestamp": "2026-03-21T10:00:00",
            "branch_coverage": false,
            "show_contexts": false
        },
        "files": {
            "taskmanager/model.py": {
                "executed_lines": [1, 2, 3, 5, 6, 10, 11],
                "missing_lines": [7, 8, 15],
                "excluded_lines": [],
                "summary": {
                    "covered_lines": 7,
                    "num_statements": 10,
                    "percent_covered": 70.0,
                    "missing_lines": 3,
                    "excluded_lines": 0
                }
            },
            "taskmanager/usecase.py": {
                "executed_lines": [1, 2, 3],
                "missing_lines": [5],
                "excluded_lines": [],
                "summary": {
                    "covered_lines": 3,
                    "num_statements": 4,
                    "percent_covered": 75.0,
                    "missing_lines": 1,
                    "excluded_lines": 0
                }
            }
        },
        "totals": {
            "covered_lines": 10,
            "num_statements": 14,
            "percent_covered": 71.4,
            "missing_lines": 4,
            "excluded_lines": 0
        }
    }"#;

    #[test]
    fn parse_basic() {
        let meta = PythonCoverageMeta {
            project_id: Some("my-python-project".into()),
            project_name: Some("My Python Project".into()),
            commit_sha: Some("abc123".into()),
            branch: Some("main".into()),
        };
        let (report, snapshot) = parse(SAMPLE_COVERAGE, &meta).unwrap();

        assert_eq!(report.format, "python-coverage");
        assert_eq!(report.project.id, "my-python-project");
        assert_eq!(report.files.len(), 2);

        // model.py: 7/10 lines
        assert_eq!(report.files[0].path, "taskmanager/model.py");
        assert_eq!(report.files[0].lines.len(), 10);
        assert!((report.files[0].line_rate - 0.7).abs() < 0.01);

        // Aggregate
        assert_eq!(snapshot.lines_covered, 10);
        assert_eq!(snapshot.lines_total, 14);
        assert_eq!(snapshot.file_count, 2);
    }

    #[test]
    fn parse_with_branches() {
        let input = r#"{
            "meta": { "format": 3, "version": "7.13.5", "branch_coverage": true },
            "files": {
                "foo.py": {
                    "executed_lines": [1, 2, 3],
                    "missing_lines": [5],
                    "excluded_lines": [],
                    "summary": {
                        "covered_lines": 3,
                        "num_statements": 4,
                        "num_branches": 4,
                        "covered_branches": 3,
                        "missing_branches": 1
                    },
                    "executed_branches": [[2, 3], [2, 5], [3, 1]],
                    "missing_branches": [[3, 5]]
                }
            },
            "totals": {
                "covered_lines": 3,
                "num_statements": 4,
                "num_branches": 4,
                "covered_branches": 3,
                "missing_branches": 1
            }
        }"#;
        let meta = PythonCoverageMeta::default();
        let (report, snapshot) = parse(input, &meta).unwrap();

        assert_eq!(snapshot.branches_total, 4);
        assert_eq!(snapshot.branches_covered, 3);
        assert!((report.files[0].branch_rate - 0.75).abs() < 0.01);
    }

    #[test]
    fn parse_empty_files_fails() {
        let input = r#"{
            "meta": { "format": 3, "version": "7.0" },
            "files": {},
            "totals": { "covered_lines": 0, "num_statements": 0 }
        }"#;
        let meta = PythonCoverageMeta::default();
        assert!(parse(input, &meta).is_err());
    }

    #[test]
    fn parse_defaults_project_info() {
        let input = r#"{
            "meta": { "format": 3, "version": "7.0" },
            "files": {
                "foo.py": {
                    "executed_lines": [1],
                    "missing_lines": [],
                    "excluded_lines": [],
                    "summary": {}
                }
            },
            "totals": { "covered_lines": 1, "num_statements": 1 }
        }"#;
        let meta = PythonCoverageMeta::default();
        let (report, _) = parse(input, &meta).unwrap();
        assert_eq!(report.project.id, "python-project");
        assert_eq!(report.project.name, "python import");
    }
}
