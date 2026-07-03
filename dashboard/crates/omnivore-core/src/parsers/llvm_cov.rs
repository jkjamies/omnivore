use crate::model::coverage::{
    source, CoverageSummary, CoverageSnapshot, CoverageTarget, FileCoverage, LineCoverage,
    OmnivoreReport, ProjectInfo,
};
use crate::parsers::{IngestMeta, ParseError};
use serde::Deserialize;

/// Metadata not present in llvm-cov export — must be supplied externally.
pub type LlvmCovMeta = IngestMeta;

// --- llvm-cov export JSON schema (subset we care about) ---

#[derive(Deserialize)]
struct LlvmCovExport {
    data: Vec<LlvmCovData>,
    #[serde(rename = "type")]
    _type: Option<String>,
    #[allow(dead_code)]
    version: Option<String>,
}

#[derive(Deserialize)]
struct LlvmCovData {
    files: Vec<LlvmCovFile>,
    totals: Option<LlvmCovTotals>,
}

#[derive(Deserialize)]
struct LlvmCovFile {
    filename: String,
    segments: Vec<Vec<serde_json::Value>>,
    summary: Option<LlvmCovTotals>,
}

#[derive(Deserialize)]
struct LlvmCovTotals {
    lines: Option<LlvmCovMetric>,
    branches: Option<LlvmCovMetric>,
}

#[derive(Deserialize)]
struct LlvmCovMetric {
    count: i64,
    covered: i64,
    percent: f64,
}

/// Parse an llvm-cov export JSON report.
///
/// Expected format: `llvm-cov export --format=json` output, which has
/// `{ "type": "llvm.coverage.json.export", "version": "...", "data": [...] }`.
///
/// Each data entry has `files` with `segments` (not line-based like lcov).
/// Segments: `[line, col, count, has_count, is_region_entry, ...]`
/// We convert segments to per-line coverage by tracking the execution count
/// at each line boundary.
pub fn parse(json: &str, meta: &LlvmCovMeta) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let export: LlvmCovExport = serde_json::from_str(json)?;

    if export.data.is_empty() {
        return Err(ParseError::LlvmCov("No data entries in export".into()));
    }

    let data = &export.data[0];
    let mut files: Vec<FileCoverage> = Vec::new();

    for file in &data.files {
        let line_coverage = segments_to_lines(&file.segments);
        if line_coverage.is_empty() {
            continue;
        }

        let total = line_coverage.len() as f64;
        let covered = line_coverage.iter().filter(|l| l.hit_count > 0).count() as f64;
        let line_rate = if total > 0.0 { covered / total } else { 0.0 };

        let branch_rate = file
            .summary
            .as_ref()
            .and_then(|s| s.branches.as_ref())
            .map(|b| b.percent / 100.0)
            .unwrap_or(0.0);

        files.push(FileCoverage {
            path: file.filename.clone(),
            line_rate,
            branch_rate,
            lines: line_coverage,
            source_content: None,
        });
    }

    if files.is_empty() {
        return Err(ParseError::LlvmCov("No files with coverage data".into()));
    }

    // Use totals from the export if available, otherwise compute from files
    let (lines_covered, lines_total, branches_covered, branches_total) =
        if let Some(totals) = &data.totals {
            let l = totals.lines.as_ref();
            let b = totals.branches.as_ref();
            (
                l.map(|m| m.covered).unwrap_or(0),
                l.map(|m| m.count).unwrap_or(0),
                b.map(|m| m.covered).unwrap_or(0),
                b.map(|m| m.count).unwrap_or(0),
            )
        } else {
            let mut lc: i64 = 0;
            let mut lt: i64 = 0;
            for f in &files {
                lt += f.lines.len() as i64;
                lc += f.lines.iter().filter(|l| l.hit_count > 0).count() as i64;
            }
            (lc, lt, 0, 0)
        };

    let line_rate = if lines_total > 0 {
        lines_covered as f64 / lines_total as f64
    } else {
        0.0
    };
    let branch_rate = if branches_total > 0 {
        branches_covered as f64 / branches_total as f64
    } else {
        0.0
    };

    let project_id = meta.project_id.clone().unwrap_or_else(|| "llvm-cov-project".into());
    let project_name = meta.project_name.clone().unwrap_or_else(|| "llvm-cov import".into());

    let report = OmnivoreReport {
        version: "0.1.0".into(),
        format: "llvm-cov".into(),
        dependencies: None,
        project: ProjectInfo {
            id: project_id,
            name: project_name,
            commit_sha: meta.commit_sha.clone(),
            branch: meta.branch.clone(),
            target: CoverageTarget::RustLlvmCov,
            source: Some(source::LLVM_COV.into()),
        },
        coverage: CoverageSummary {
            line_rate,
            branch_rate,
            lines_covered,
            lines_total,
            branches_covered,
            branches_total,
        },
        files,
    };

    let snapshot = CoverageSnapshot::from_report(&report, Some(source::LLVM_COV));
    Ok((report, snapshot))
}

/// Convert llvm-cov segments to per-line coverage.
///
/// Segments are `[line, column, count, has_count, is_region_entry, ...]`.
/// We walk through segments and track the current execution count,
/// emitting one LineCoverage per line that has `has_count = true`.
fn segments_to_lines(segments: &[Vec<serde_json::Value>]) -> Vec<LineCoverage> {
    use std::collections::BTreeMap;

    let mut line_counts: BTreeMap<i32, i64> = BTreeMap::new();

    for seg in segments {
        if seg.len() < 4 {
            continue;
        }
        let line = match seg[0].as_i64() {
            Some(l) => l as i32,
            None => continue,
        };
        let count = match seg[2].as_i64() {
            Some(c) => c,
            None => continue,
        };
        let has_count = match seg[3].as_bool().or_else(|| seg[3].as_i64().map(|n| n != 0)) {
            Some(true) => true,
            _ => continue,
        };

        if has_count {
            // Take the max count if a line appears in multiple segments
            let entry = line_counts.entry(line).or_insert(0);
            *entry = (*entry).max(count);
        }
    }

    line_counts
        .into_iter()
        .map(|(line_number, hit_count)| LineCoverage { line_number, hit_count })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LLVM_COV: &str = r#"{
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
                            [5, 1, 3, true, true],
                            [7, 1, 0, false, false]
                        ],
                        "summary": {
                            "lines": { "count": 3, "covered": 2, "percent": 66.67 },
                            "branches": { "count": 2, "covered": 1, "percent": 50.0 }
                        }
                    },
                    {
                        "filename": "src/lib.rs",
                        "segments": [
                            [1, 1, 10, true, true],
                            [2, 1, 10, true, true],
                            [3, 1, 0, false, false]
                        ],
                        "summary": {
                            "lines": { "count": 2, "covered": 2, "percent": 100.0 },
                            "branches": { "count": 0, "covered": 0, "percent": 0.0 }
                        }
                    }
                ],
                "totals": {
                    "lines": { "count": 5, "covered": 4, "percent": 80.0 },
                    "branches": { "count": 2, "covered": 1, "percent": 50.0 }
                }
            }
        ]
    }"#;

    #[test]
    fn parse_llvm_cov_basic() {
        let meta = LlvmCovMeta {
            project_id: Some("rust-app".into()),
            project_name: Some("Rust App".into()),
            commit_sha: Some("def456".into()),
            branch: Some("main".into()),
        };
        let (report, snapshot) = parse(SAMPLE_LLVM_COV, &meta).unwrap();

        assert_eq!(report.format, "llvm-cov");
        assert_eq!(report.project.id, "rust-app");
        assert_eq!(report.files.len(), 2);

        // First file: 3 segments with has_count=true → lines 1(5), 3(0), 5(3)
        assert_eq!(report.files[0].path, "src/main.rs");
        assert_eq!(report.files[0].lines.len(), 3);

        // Totals from export
        assert_eq!(snapshot.lines_covered, 4);
        assert_eq!(snapshot.lines_total, 5);
        assert_eq!(snapshot.branches_covered, 1);
        assert_eq!(snapshot.branches_total, 2);
        assert_eq!(snapshot.file_count, 2);
    }

    #[test]
    fn parse_llvm_cov_empty_data_fails() {
        let json = r#"{"type":"llvm.coverage.json.export","version":"2.0.1","data":[]}"#;
        let meta = LlvmCovMeta::default();
        assert!(parse(json, &meta).is_err());
    }

    #[test]
    fn parse_llvm_cov_defaults_project_info() {
        let json = r#"{
            "data": [{
                "files": [{
                    "filename": "main.go",
                    "segments": [[1,1,1,true,true],[2,1,0,false,false]]
                }]
            }]
        }"#;
        let meta = LlvmCovMeta::default();
        let (report, _) = parse(json, &meta).unwrap();
        assert_eq!(report.project.id, "llvm-cov-project");
        assert_eq!(report.project.name, "llvm-cov import");
    }
}
