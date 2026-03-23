use crate::model::coverage::{
    CoverageSummary, CoverageSnapshot, CoverageTarget, FileCoverage, LineCoverage,
    OmnivoreReport, ProjectInfo,
};
use crate::parsers::ParseError;
use chrono::Utc;
use uuid::Uuid;

/// Metadata not present in lcov format — must be supplied externally.
#[derive(Debug, Clone, Default)]
pub struct LcovMeta {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
}

/// Parse an lcov-format coverage report.
///
/// lcov records:
/// - `TN:<test name>` — test name (ignored)
/// - `SF:<path>` — start of source file
/// - `DA:<line>,<count>` — line execution count
/// - `BRDA:<line>,<block>,<branch>,<taken>` — branch data
/// - `LF:<count>` — lines found
/// - `LH:<count>` — lines hit
/// - `BRF:<count>` — branches found
/// - `BRH:<count>` — branches hit
/// - `end_of_record` — end of source file block
pub fn parse(input: &str, meta: &LcovMeta) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let mut files: Vec<FileCoverage> = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_lines: Vec<LineCoverage> = Vec::new();

    // Aggregate branch counters per file
    let mut total_branches_found: i64 = 0;
    let mut total_branches_hit: i64 = 0;
    let mut file_brf: i64 = 0;
    let mut file_brh: i64 = 0;

    for (line_num, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("TN:") {
            // Test name — ignored
            continue;
        }

        if let Some(path) = line.strip_prefix("SF:") {
            // Start of a new source file
            if current_path.is_some() {
                // Flush previous file (missing end_of_record)
                flush_file(
                    &mut files,
                    &mut current_path,
                    &mut current_lines,
                    &mut total_branches_found,
                    &mut total_branches_hit,
                    file_brf,
                    file_brh,
                );
            }
            current_path = Some(path.to_string());
            current_lines.clear();
            file_brf = 0;
            file_brh = 0;
            continue;
        }

        if let Some(rest) = line.strip_prefix("DA:") {
            let parts: Vec<&str> = rest.splitn(3, ',').collect();
            if parts.len() < 2 {
                return Err(ParseError::Lcov(format!("Invalid DA record at line {}", line_num + 1)));
            }
            let line_number: i32 = parts[0]
                .parse()
                .map_err(|_| ParseError::Lcov(format!("Bad line number in DA at line {}", line_num + 1)))?;
            let hit_count: i64 = parts[1]
                .parse()
                .map_err(|_| ParseError::Lcov(format!("Bad hit count in DA at line {}", line_num + 1)))?;
            current_lines.push(LineCoverage { line_number, hit_count });
            continue;
        }

        if let Some(rest) = line.strip_prefix("BRDA:") {
            // BRDA:<line>,<block>,<branch>,<taken>
            // taken can be "-" (not executed) or a number
            let parts: Vec<&str> = rest.splitn(4, ',').collect();
            if parts.len() == 4 {
                file_brf += 1;
                if parts[3] != "-" {
                    if let Ok(taken) = parts[3].parse::<i64>() {
                        if taken > 0 {
                            file_brh += 1;
                        }
                    }
                }
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("BRF:") {
            if let Ok(n) = rest.parse::<i64>() {
                file_brf = n;
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("BRH:") {
            if let Ok(n) = rest.parse::<i64>() {
                file_brh = n;
            }
            continue;
        }

        if line == "end_of_record" {
            flush_file(
                &mut files,
                &mut current_path,
                &mut current_lines,
                &mut total_branches_found,
                &mut total_branches_hit,
                file_brf,
                file_brh,
            );
            file_brf = 0;
            file_brh = 0;
            continue;
        }

        // FN, FNDA, FNF, FNH, LF, LH — we derive these from DA records, skip
    }

    // Flush last file if missing end_of_record
    if current_path.is_some() {
        flush_file(
            &mut files,
            &mut current_path,
            &mut current_lines,
            &mut total_branches_found,
            &mut total_branches_hit,
            file_brf,
            file_brh,
        );
    }

    if files.is_empty() {
        return Err(ParseError::Lcov("No source files found in lcov data".into()));
    }

    // Compute aggregate coverage
    let mut lines_covered: i64 = 0;
    let mut lines_total: i64 = 0;
    for f in &files {
        lines_total += f.lines.len() as i64;
        lines_covered += f.lines.iter().filter(|l| l.hit_count > 0).count() as i64;
    }

    let line_rate = if lines_total > 0 {
        lines_covered as f64 / lines_total as f64
    } else {
        0.0
    };
    let branch_rate = if total_branches_found > 0 {
        total_branches_hit as f64 / total_branches_found as f64
    } else {
        0.0
    };

    let project_id = meta.project_id.clone().unwrap_or_else(|| "lcov-project".into());
    let project_name = meta.project_name.clone().unwrap_or_else(|| "lcov import".into());

    let report = OmnivoreReport {
        version: "0.1.0".into(),
        format: "lcov".into(),
        dependencies: None,
        project: ProjectInfo {
            id: project_id,
            name: project_name,
            commit_sha: meta.commit_sha.clone(),
            branch: meta.branch.clone(),
            target: CoverageTarget::Lcov,
        },
        coverage: CoverageSummary {
            line_rate,
            branch_rate,
            lines_covered,
            lines_total,
            branches_covered: total_branches_hit,
            branches_total: total_branches_found,
        },
        files,
    };

    let files_json = serde_json::to_string(&report.files).ok();
    let snapshot = CoverageSnapshot {
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
        dependencies_json: None,
    };

    Ok((report, snapshot))
}

fn flush_file(
    files: &mut Vec<FileCoverage>,
    current_path: &mut Option<String>,
    current_lines: &mut Vec<LineCoverage>,
    total_branches_found: &mut i64,
    total_branches_hit: &mut i64,
    file_brf: i64,
    file_brh: i64,
) {
    if let Some(path) = current_path.take() {
        let lines_total = current_lines.len() as f64;
        let lines_hit = current_lines.iter().filter(|l| l.hit_count > 0).count() as f64;
        let line_rate = if lines_total > 0.0 { lines_hit / lines_total } else { 0.0 };
        let branch_rate = if file_brf > 0 { file_brh as f64 / file_brf as f64 } else { 0.0 };

        *total_branches_found += file_brf;
        *total_branches_hit += file_brh;

        files.push(FileCoverage {
            path,
            line_rate,
            branch_rate,
            lines: std::mem::take(current_lines),
            source_content: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LCOV: &str = "\
TN:test-suite
SF:src/main.rs
DA:1,1
DA:2,1
DA:3,0
DA:5,1
BRDA:2,0,0,1
BRDA:2,0,1,0
BRF:2
BRH:1
LF:4
LH:3
end_of_record
SF:src/lib.rs
DA:1,1
DA:2,0
LF:2
LH:1
end_of_record
";

    #[test]
    fn parse_lcov_basic() {
        let meta = LcovMeta {
            project_id: Some("my-project".into()),
            project_name: Some("My Project".into()),
            commit_sha: Some("abc123".into()),
            branch: Some("main".into()),
        };
        let (report, snapshot) = parse(SAMPLE_LCOV, &meta).unwrap();

        assert_eq!(report.format, "lcov");
        assert_eq!(report.project.id, "my-project");
        assert_eq!(report.files.len(), 2);

        // First file: 3/4 lines hit
        assert_eq!(report.files[0].path, "src/main.rs");
        assert_eq!(report.files[0].lines.len(), 4);
        assert!((report.files[0].line_rate - 0.75).abs() < 0.01);
        assert!((report.files[0].branch_rate - 0.5).abs() < 0.01);

        // Second file: 1/2 lines hit
        assert_eq!(report.files[1].path, "src/lib.rs");
        assert_eq!(report.files[1].lines.len(), 2);
        assert!((report.files[1].line_rate - 0.5).abs() < 0.01);

        // Aggregate: 4/6 lines covered
        assert_eq!(snapshot.lines_covered, 4);
        assert_eq!(snapshot.lines_total, 6);
        assert_eq!(snapshot.branches_covered, 1);
        assert_eq!(snapshot.branches_total, 2);
        assert_eq!(snapshot.file_count, 2);
    }

    #[test]
    fn parse_lcov_empty_fails() {
        let meta = LcovMeta::default();
        let result = parse("", &meta);
        assert!(result.is_err());
    }

    #[test]
    fn parse_lcov_defaults_project_info() {
        let lcov = "SF:foo.go\nDA:1,1\nend_of_record\n";
        let meta = LcovMeta::default();
        let (report, _) = parse(lcov, &meta).unwrap();
        assert_eq!(report.project.id, "lcov-project");
        assert_eq!(report.project.name, "lcov import");
    }
}
