mod dependencies;
mod file_coverage;
mod health_page;
mod project_detail;
mod projects;

// Re-export all public handlers
pub use dependencies::dependency_graph_page;
pub use file_coverage::{file_coverage_page, file_source_fragment};
pub use health_page::health_page;
pub use project_detail::project_detail_page;
pub use projects::projects_page;

// -- Shared helpers used across page modules --

use omnivore_core::model::coverage::FileCoverage;

/// A node in the file tree: either a directory (with children) or a leaf file.
pub struct FileTreeNode {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub file_count: usize,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
}

/// Summary data for a single coverage target.
pub struct TargetSnapshot {
    pub target: String,
    pub label: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
    pub files: Vec<FileCoverage>,
    pub file_tree: Vec<FileTreeNode>,
    pub trend: Vec<TrendEntry>,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
}

impl TargetSnapshot {
    pub fn from_snapshot(
        snap: &omnivore_core::model::coverage::CoverageSnapshot,
        prev: Option<&omnivore_core::model::coverage::CoverageSnapshot>,
        trend: Vec<TrendEntry>,
    ) -> Self {
        let files: Vec<FileCoverage> = snap
            .files_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

        let prev_files: std::collections::HashMap<String, (f64, f64)> = prev
            .and_then(|p| p.files_json.as_ref())
            .and_then(|json| serde_json::from_str::<Vec<FileCoverage>>(json).ok())
            .map(|pf| pf.into_iter().map(|f| (f.path.clone(), (f.line_rate, f.branch_rate))).collect())
            .unwrap_or_default();

        let file_tree = build_file_tree(&files, &prev_files);

        let line_delta = prev.map(|p| snap.line_rate - p.line_rate);
        let branch_delta = prev.map(|p| snap.branch_rate - p.branch_rate);

        Self {
            target: snap.target.clone(),
            label: target_label(&snap.target),
            line_rate: snap.line_rate,
            branch_rate: snap.branch_rate,
            lines_covered: snap.lines_covered,
            lines_total: snap.lines_total,
            branches_covered: snap.branches_covered,
            branches_total: snap.branches_total,
            file_count: snap.file_count,
            files,
            file_tree,
            trend,
            line_delta,
            branch_delta,
        }
    }
}

/// Composite summary computed from multiple targets.
pub struct CompositeSnapshot {
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
}

#[derive(serde::Serialize, Clone)]
pub struct TrendEntry {
    pub line_rate: f64,
    pub branch_rate: f64,
    pub created_at: String,
}

pub struct HotspotFile {
    pub path: String,
    pub line_rate: f64,
    pub uncovered_lines: i64,
    pub total_lines: i64,
}

// -- Pure functions --

pub fn fmt_pct_val(rate: f64) -> String {
    format!("{:.1}", rate * 100.0)
}

pub fn rate_color_with_threshold(rate: f64, threshold: f64, warn_threshold: f64) -> &'static str {
    if rate >= threshold {
        "var(--green)"
    } else if rate >= warn_threshold {
        "var(--yellow)"
    } else {
        "var(--red)"
    }
}

pub fn rate_color_val(rate: f64) -> &'static str {
    rate_color_with_threshold(rate, 0.8, 0.5)
}

pub fn fmt_delta_html(delta: Option<f64>) -> String {
    match delta {
        None => String::new(),
        Some(d) => {
            let pct = d * 100.0;
            if pct.abs() < 0.05 {
                String::new()
            } else if pct > 0.0 {
                format!(r#"<span class="delta delta-up">(+{:.1}%)</span>"#, pct)
            } else {
                format!(r#"<span class="delta delta-down">({:.1}%)</span>"#, pct)
            }
        }
    }
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn target_label(target: &str) -> String {
    match target {
        "JVM_UNIT" | "JvmUnit" => "Unit Tests".to_string(),
        "ANDROID_INSTRUMENTED" | "AndroidInstrumented" => "Instrumented Tests".to_string(),
        "IOS_UNIT" | "IosUnit" => "iOS Unit Tests".to_string(),
        "KOTLIN_NATIVE" | "KotlinNative" => "Kotlin/Native Tests".to_string(),
        "COMPOSITE" | "Composite" => "Composite".to_string(),
        "RUST_LLVM_COV" | "RustLlvmCov" => "Rust (llvm-cov)".to_string(),
        "GO_COVER" | "GoCover" => "Go".to_string(),
        "PYTHON_COVERAGE" | "PythonCoverage" => "Python".to_string(),
        "LCOV" | "Lcov" => "lcov".to_string(),
        other => other.to_string(),
    }
}

pub fn compute_composite(targets: &[TargetSnapshot]) -> Option<CompositeSnapshot> {
    if targets.len() < 2 {
        return None;
    }
    let lines_covered: i64 = targets.iter().map(|t| t.lines_covered).sum();
    let lines_total: i64 = targets.iter().map(|t| t.lines_total).sum();
    let branches_covered: i64 = targets.iter().map(|t| t.branches_covered).sum();
    let branches_total: i64 = targets.iter().map(|t| t.branches_total).sum();
    let file_count: i64 = targets.iter().map(|t| t.file_count).sum();
    let line_rate = if lines_total > 0 { lines_covered as f64 / lines_total as f64 } else { 0.0 };
    let branch_rate = if branches_total > 0 { branches_covered as f64 / branches_total as f64 } else { 0.0 };

    let line_delta = if targets.iter().all(|t| t.line_delta.is_some()) {
        let prev_line_rate = targets.iter()
            .map(|t| {
                let prev_covered = t.lines_covered as f64 - t.line_delta.unwrap() * t.lines_total as f64;
                (prev_covered, t.lines_total as f64)
            })
            .fold((0.0, 0.0), |acc, (c, t)| (acc.0 + c, acc.1 + t));
        if prev_line_rate.1 > 0.0 {
            Some(line_rate - prev_line_rate.0 / prev_line_rate.1)
        } else {
            None
        }
    } else {
        None
    };

    let branch_delta = if targets.iter().all(|t| t.branch_delta.is_some()) {
        let prev_branch_rate = targets.iter()
            .map(|t| {
                let prev_covered = t.branches_covered as f64 - t.branch_delta.unwrap() * t.branches_total as f64;
                (prev_covered, t.branches_total as f64)
            })
            .fold((0.0, 0.0), |acc, (c, t)| (acc.0 + c, acc.1 + t));
        if prev_branch_rate.1 > 0.0 {
            Some(branch_rate - prev_branch_rate.0 / prev_branch_rate.1)
        } else {
            None
        }
    } else {
        None
    };

    Some(CompositeSnapshot {
        line_rate, branch_rate, lines_covered, lines_total,
        branches_covered, branches_total, file_count, line_delta, branch_delta,
    })
}

// -- File tree building --

pub fn build_file_tree(files: &[FileCoverage], prev_rates: &std::collections::HashMap<String, (f64, f64)>) -> Vec<FileTreeNode> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    let mut root_files: Vec<&FileCoverage> = Vec::new();

    for f in files {
        if let Some(idx) = f.path.find('/') {
            let dir = f.path[..idx].to_string();
            groups.entry(dir).or_default().push(f);
        } else {
            root_files.push(f);
        }
    }

    let mut nodes = Vec::new();
    for (dir_name, dir_files) in &groups {
        nodes.push(build_dir_node(dir_name, "", dir_files, prev_rates));
    }
    for f in root_files {
        let prev = prev_rates.get(&f.path);
        nodes.push(FileTreeNode {
            name: f.path.clone(),
            full_path: f.path.clone(),
            is_dir: false,
            children: vec![],
            line_rate: f.line_rate,
            branch_rate: f.branch_rate,
            file_count: 1,
            line_delta: prev.map(|(lr, _)| f.line_rate - lr),
            branch_delta: prev.map(|(_, br)| f.branch_rate - br),
        });
    }
    nodes
}

fn build_dir_node(dir_name: &str, parent_path: &str, files: &[&FileCoverage], prev_rates: &std::collections::HashMap<String, (f64, f64)>) -> FileTreeNode {
    use std::collections::BTreeMap;

    let full_dir = if parent_path.is_empty() {
        dir_name.to_string()
    } else {
        format!("{}/{}", parent_path, dir_name)
    };
    let prefix = format!("{}/", full_dir);

    let mut sub_groups: BTreeMap<String, Vec<&FileCoverage>> = BTreeMap::new();
    let mut leaf_files: Vec<&FileCoverage> = Vec::new();

    for f in files {
        let relative = f.path.strip_prefix(&prefix).unwrap_or(&f.path);
        if let Some(idx) = relative.find('/') {
            let sub_dir = relative[..idx].to_string();
            sub_groups.entry(sub_dir).or_default().push(f);
        } else {
            leaf_files.push(f);
        }
    }

    if sub_groups.len() == 1 && leaf_files.is_empty() {
        let (sub_name, sub_files) = sub_groups.into_iter().next().unwrap();
        let collapsed_name = format!("{}/{}", dir_name, sub_name);
        return build_dir_node(&collapsed_name, parent_path, &sub_files, prev_rates);
    }

    let mut children = Vec::new();
    for (sub_name, sub_files) in &sub_groups {
        children.push(build_dir_node(sub_name, &full_dir, sub_files, prev_rates));
    }
    for f in &leaf_files {
        let file_name = f.path.rsplit('/').next().unwrap_or(&f.path).to_string();
        let prev = prev_rates.get(&f.path);
        children.push(FileTreeNode {
            name: file_name,
            full_path: f.path.clone(),
            is_dir: false,
            children: vec![],
            line_rate: f.line_rate,
            branch_rate: f.branch_rate,
            file_count: 1,
            line_delta: prev.map(|(lr, _)| f.line_rate - lr),
            branch_delta: prev.map(|(_, br)| f.branch_rate - br),
        });
    }

    let total_lines: i64 = files.iter().map(|f| f.lines.len() as i64).sum();
    let covered_lines: i64 = files.iter().map(|f| f.lines.iter().filter(|l| l.hit_count > 0).count() as i64).sum();
    let line_rate = if total_lines > 0 { covered_lines as f64 / total_lines as f64 } else { 0.0 };

    let total_branches: f64 = files.len() as f64;
    let branch_rate_sum: f64 = files.iter().map(|f| f.branch_rate).sum();
    let branch_rate = if total_branches > 0.0 { branch_rate_sum / total_branches } else { 0.0 };

    let line_delta = if children.iter().any(|c| c.line_delta.is_some()) {
        Some(children.iter().filter_map(|c| c.line_delta).sum::<f64>() / children.len() as f64)
    } else {
        None
    };
    let branch_delta = if children.iter().any(|c| c.branch_delta.is_some()) {
        Some(children.iter().filter_map(|c| c.branch_delta).sum::<f64>() / children.len() as f64)
    } else {
        None
    };

    FileTreeNode {
        name: dir_name.to_string(),
        full_path: full_dir,
        is_dir: true,
        children,
        line_rate,
        branch_rate,
        file_count: files.len(),
        line_delta,
        branch_delta,
    }
}
