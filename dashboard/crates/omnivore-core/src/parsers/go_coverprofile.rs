use crate::model::coverage::{
    source, CoverageSummary, CoverageSnapshot, CoverageTarget, FileCoverage, LineCoverage,
    OmnivoreReport, ProjectInfo,
};
use crate::parsers::{IngestMeta, ParseError};

/// Metadata not present in Go coverprofile — must be supplied externally.
pub type GoCoverprofileMeta = IngestMeta;

/// Parse a Go coverprofile into an Omnivore report.
///
/// Go coverprofile format:
/// ```text
/// mode: count
/// github.com/foo/bar/pkg/file.go:10.2,15.3 2 5
/// ```
/// Each line after the mode header is: `file:startLine.startCol,endLine.endCol numStatements count`
///
/// We expand each block into per-line DA-style records, taking the max count when
/// blocks overlap on the same line.
pub fn parse(input: &str, meta: &GoCoverprofileMeta) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    // Map: filename -> (line_number -> max_count)
    let mut files: std::collections::HashMap<String, std::collections::HashMap<i32, i64>> =
        std::collections::HashMap::new();

    let mut has_mode_line = false;

    for raw_line in input.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("mode:") {
            has_mode_line = true;
            continue;
        }

        // Parse: file:startLine.startCol,endLine.endCol numStatements count
        let colon_idx = match line.rfind(':') {
            Some(i) => i,
            None => continue,
        };
        let file_name = &line[..colon_idx];
        let rest = &line[colon_idx + 1..];

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let range_part = parts[0];
        let count: i64 = match parts[parts.len() - 1].parse() {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Parse "startLine.startCol,endLine.endCol"
        let range_parts: Vec<&str> = range_part.split(',').collect();
        if range_parts.len() != 2 {
            continue;
        }
        let start_line = parse_line_num(range_parts[0]);
        let end_line = parse_line_num(range_parts[1]);

        let file_lines = files.entry(file_name.to_string()).or_default();
        for ln in start_line..=end_line {
            let existing = file_lines.entry(ln).or_insert(0);
            if count > *existing {
                *existing = count;
            }
        }
    }

    if !has_mode_line {
        return Err(ParseError::GoCoverprofile("Missing mode: header".into()));
    }
    if files.is_empty() {
        return Err(ParseError::GoCoverprofile("No coverage data found".into()));
    }

    // Strip common module prefix to get relative paths
    let prefix = find_common_prefix(&files);

    // Build FileCoverage entries
    let mut file_coverages: Vec<FileCoverage> = Vec::new();
    let mut sorted_files: Vec<String> = files.keys().cloned().collect();
    sorted_files.sort();

    let mut total_lines: i64 = 0;
    let mut total_hit: i64 = 0;

    for file_name in &sorted_files {
        let line_map = &files[file_name];
        let rel_path = file_name.strip_prefix(&prefix).unwrap_or(file_name);

        let mut sorted_lines: Vec<i32> = line_map.keys().copied().collect();
        sorted_lines.sort();

        let mut lines: Vec<LineCoverage> = Vec::new();
        let mut file_hit = 0i64;
        for &ln in &sorted_lines {
            let count = line_map[&ln];
            lines.push(LineCoverage {
                line_number: ln,
                hit_count: count,
            });
            if count > 0 {
                file_hit += 1;
            }
        }

        let file_total = lines.len() as f64;
        let line_rate = if file_total > 0.0 {
            file_hit as f64 / file_total
        } else {
            0.0
        };

        total_lines += lines.len() as i64;
        total_hit += file_hit;

        file_coverages.push(FileCoverage {
            path: rel_path.to_string(),
            line_rate,
            branch_rate: 0.0, // Go coverprofile doesn't have branch data
            lines,
            source_content: None,
        });
    }

    let line_rate = if total_lines > 0 {
        total_hit as f64 / total_lines as f64
    } else {
        0.0
    };

    let project_id = meta.project_id.clone().unwrap_or_else(|| "go-project".into());
    let project_name = meta.project_name.clone().unwrap_or_else(|| "go import".into());

    let report = OmnivoreReport {
        version: "0.1.0".into(),
        format: "go-coverprofile".into(),
        dependencies: None,
        project: ProjectInfo {
            id: project_id,
            name: project_name,
            commit_sha: meta.commit_sha.clone(),
            branch: meta.branch.clone(),
            target: CoverageTarget::GoCover,
            source: Some(source::GO.into()),
        },
        coverage: CoverageSummary {
            line_rate,
            branch_rate: 0.0,
            lines_covered: total_hit,
            lines_total: total_lines,
            branches_covered: 0,
            branches_total: 0,
        },
        files: file_coverages,
    };

    let snapshot = CoverageSnapshot::from_report(&report, Some(source::GO));
    Ok((report, snapshot))
}

fn parse_line_num(s: &str) -> i32 {
    let dot_idx = s.find('.');
    let num_part = match dot_idx {
        Some(i) => &s[..i],
        None => s,
    };
    num_part.parse().unwrap_or(0)
}

fn find_common_prefix(files: &std::collections::HashMap<String, std::collections::HashMap<i32, i64>>) -> String {
    let names: Vec<&str> = files.keys().map(|s| s.as_str()).collect();
    if names.is_empty() {
        return String::new();
    }
    let first = names[0];
    let mut last_valid = 0;
    for (i, ch) in first.char_indices() {
        if ch != '/' {
            continue;
        }
        let prefix = &first[..=i];
        if names.iter().all(|n| n.starts_with(prefix)) {
            last_valid = i + 1;
        } else {
            break;
        }
    }
    first[..last_valid].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_COVERPROFILE: &str = "\
mode: count
github.com/jkjamies/omnivore/go-test-rig/model/task.go:10.2,15.3 2 5
github.com/jkjamies/omnivore/go-test-rig/model/task.go:17.1,20.2 1 0
github.com/jkjamies/omnivore/go-test-rig/usecase/usecase.go:5.2,8.3 1 3
";

    #[test]
    fn parse_basic() {
        let meta = GoCoverprofileMeta {
            project_id: Some("my-go-project".into()),
            project_name: Some("My Go Project".into()),
            commit_sha: Some("def456".into()),
            branch: Some("main".into()),
        };
        let (report, snapshot) = parse(SAMPLE_COVERPROFILE, &meta).unwrap();

        assert_eq!(report.format, "go-coverprofile");
        assert_eq!(report.project.id, "my-go-project");
        assert_eq!(report.files.len(), 2);

        // model/task.go: lines 10-15 hit (count=5), lines 17-20 not hit (count=0)
        assert_eq!(report.files[0].path, "model/task.go");
        let task_lines = &report.files[0].lines;
        assert!(task_lines.iter().any(|l| l.line_number == 10 && l.hit_count == 5));
        assert!(task_lines.iter().any(|l| l.line_number == 17 && l.hit_count == 0));

        // usecase/usecase.go: lines 5-8 hit (count=3)
        assert_eq!(report.files[1].path, "usecase/usecase.go");

        // Snapshot aggregates
        assert_eq!(snapshot.project_id, "my-go-project");
        assert!(snapshot.lines_covered > 0);
        assert!(snapshot.lines_total > 0);
        assert_eq!(snapshot.branches_total, 0); // Go has no branch data
    }

    #[test]
    fn parse_missing_mode_fails() {
        let meta = GoCoverprofileMeta::default();
        let result = parse("some/file.go:1.1,2.1 1 1", &meta);
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_data_fails() {
        let meta = GoCoverprofileMeta::default();
        let result = parse("mode: count\n", &meta);
        assert!(result.is_err());
    }

    #[test]
    fn parse_defaults_project_info() {
        let input = "mode: count\nfoo.go:1.1,2.1 1 1\n";
        let meta = GoCoverprofileMeta::default();
        let (report, _) = parse(input, &meta).unwrap();
        assert_eq!(report.project.id, "go-project");
        assert_eq!(report.project.name, "go import");
    }

    #[test]
    fn parse_overlapping_blocks_takes_max() {
        let input = "\
mode: count
foo.go:1.1,5.1 2 3
foo.go:3.1,7.1 2 10
";
        let meta = GoCoverprofileMeta::default();
        let (report, _) = parse(input, &meta).unwrap();
        let lines = &report.files[0].lines;
        // Line 3 should have max(3, 10) = 10
        let line3 = lines.iter().find(|l| l.line_number == 3).unwrap();
        assert_eq!(line3.hit_count, 10);
        // Line 1 should have 3 (only from first block)
        let line1 = lines.iter().find(|l| l.line_number == 1).unwrap();
        assert_eq!(line1.hit_count, 3);
    }
}
