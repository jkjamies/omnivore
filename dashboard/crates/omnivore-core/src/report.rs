use serde::Serialize;

use crate::model::coverage::{CoverageSnapshot, FileCoverage};
use crate::model::project::Project;

/// A complete export report for a project — mirrors the project detail page summary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub project_name: String,
    pub project_id: String,
    pub current: SnapshotInfo,
    pub baseline: Option<SnapshotInfo>,
    pub targets: Vec<TargetReport>,
    pub overview: CoverageOverview,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub date: String,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetReport {
    pub target: String,
    pub label: String,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
    pub branches_covered: i64,
    pub branches_total: i64,
    pub file_count: i64,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
    pub status: String,
}

/// Aggregate overview across all targets.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageOverview {
    pub total_files: i64,
    pub total_lines: i64,
    pub total_lines_covered: i64,
    pub total_branches: i64,
    pub total_branches_covered: i64,
    pub overall_line_rate: f64,
    pub overall_branch_rate: f64,
    pub overall_line_delta: Option<f64>,
    pub overall_branch_delta: Option<f64>,
    pub files_with_zero_coverage: usize,
    pub files_below_50_pct: usize,
    pub files_above_80_pct: usize,
    pub status: String,
}

fn target_label(target: &str) -> &str {
    match target {
        "JVM_UNIT" | "JvmUnit" => "Unit Tests",
        "ANDROID_INSTRUMENTED" | "AndroidInstrumented" => "Instrumented Tests",
        "IOS_UNIT" | "IosUnit" => "iOS Unit Tests",
        "KOTLIN_NATIVE" | "KotlinNative" => "Kotlin/Native Tests",
        "COMPOSITE" | "Composite" => "Composite",
        "RUST_LLVM_COV" | "RustLlvmCov" => "Rust (llvm-cov)",
        "GO_COVER" | "GoCover" => "Go",
        "PYTHON_COVERAGE" | "PythonCoverage" => "Python",
        "LCOV" | "Lcov" => "lcov",
        other => other,
    }
}

fn status_label(rate: f64) -> &'static str {
    if rate >= 0.8 {
        "Passing"
    } else if rate >= 0.5 {
        "Warning"
    } else {
        "Failing"
    }
}

fn parse_files(snap: &CoverageSnapshot) -> Vec<FileCoverage> {
    snap.files_json
        .as_ref()
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default()
}

/// Build a report comparing current snapshots against baseline snapshots.
pub fn build_export_report(
    project: &Project,
    current_snapshots: &[CoverageSnapshot],
    baseline_snapshots: &[Option<CoverageSnapshot>],
) -> ExportReport {
    let first = current_snapshots.first();

    let current_info = SnapshotInfo {
        date: first
            .map(|s| s.created_at.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_default(),
        commit_sha: first.and_then(|s| s.commit_sha.clone()),
        branch: first.and_then(|s| s.branch.clone()),
    };

    let baseline_info = baseline_snapshots.iter().flatten().next().map(|s| SnapshotInfo {
        date: s.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        commit_sha: s.commit_sha.clone(),
        branch: s.branch.clone(),
    });

    let mut targets = Vec::new();
    let mut total_lines: i64 = 0;
    let mut total_lines_covered: i64 = 0;
    let mut total_branches: i64 = 0;
    let mut total_branches_covered: i64 = 0;
    let mut total_files: i64 = 0;
    let mut all_files: Vec<FileCoverage> = Vec::new();

    for (i, snap) in current_snapshots.iter().enumerate() {
        let baseline = baseline_snapshots.get(i).and_then(|b| b.as_ref());

        let line_delta = baseline.map(|b| snap.line_rate - b.line_rate);
        let branch_delta = baseline.map(|b| snap.branch_rate - b.branch_rate);

        targets.push(TargetReport {
            target: snap.target.clone(),
            label: target_label(&snap.target).to_string(),
            line_rate: snap.line_rate,
            branch_rate: snap.branch_rate,
            lines_covered: snap.lines_covered,
            lines_total: snap.lines_total,
            branches_covered: snap.branches_covered,
            branches_total: snap.branches_total,
            file_count: snap.file_count,
            line_delta,
            branch_delta,
            status: status_label(snap.line_rate).to_string(),
        });

        total_lines += snap.lines_total;
        total_lines_covered += snap.lines_covered;
        total_branches += snap.branches_total;
        total_branches_covered += snap.branches_covered;
        total_files += snap.file_count;

        all_files.extend(parse_files(snap));
    }

    let overall_line_rate = if total_lines > 0 {
        total_lines_covered as f64 / total_lines as f64
    } else {
        0.0
    };
    let overall_branch_rate = if total_branches > 0 {
        total_branches_covered as f64 / total_branches as f64
    } else {
        0.0
    };

    // Compute overall deltas from baseline
    let (overall_line_delta, overall_branch_delta) = if baseline_snapshots.iter().any(|b| b.is_some()) {
        let mut bl_lines: i64 = 0;
        let mut bl_covered: i64 = 0;
        let mut bl_branches: i64 = 0;
        let mut bl_br_covered: i64 = 0;
        for b in baseline_snapshots.iter().flatten() {
            bl_lines += b.lines_total;
            bl_covered += b.lines_covered;
            bl_branches += b.branches_total;
            bl_br_covered += b.branches_covered;
        }
        let bl_line_rate = if bl_lines > 0 { bl_covered as f64 / bl_lines as f64 } else { 0.0 };
        let bl_branch_rate = if bl_branches > 0 { bl_br_covered as f64 / bl_branches as f64 } else { 0.0 };
        (Some(overall_line_rate - bl_line_rate), Some(overall_branch_rate - bl_branch_rate))
    } else {
        (None, None)
    };

    // File distribution counts
    let files_with_zero_coverage = all_files.iter().filter(|f| f.line_rate < 0.005).count();
    let files_below_50_pct = all_files.iter().filter(|f| f.line_rate < 0.5).count();
    let files_above_80_pct = all_files.iter().filter(|f| f.line_rate >= 0.8).count();

    ExportReport {
        project_name: project.name.clone(),
        project_id: project.id.clone(),
        current: current_info,
        baseline: baseline_info,
        targets,
        overview: CoverageOverview {
            total_files,
            total_lines,
            total_lines_covered,
            total_branches,
            total_branches_covered,
            overall_line_rate,
            overall_branch_rate,
            overall_line_delta,
            overall_branch_delta,
            files_with_zero_coverage,
            files_below_50_pct,
            files_above_80_pct,
            status: status_label(overall_line_rate).to_string(),
        },
    }
}

fn fmt_pct(rate: f64) -> String {
    format!("{:.1}%", rate * 100.0)
}

fn fmt_delta(delta: Option<f64>) -> String {
    match delta {
        None => String::new(),
        Some(d) => {
            let pct = d * 100.0;
            if pct.abs() < 0.05 {
                String::new()
            } else if pct > 0.0 {
                format!(" (+{:.1}%)", pct)
            } else {
                format!(" ({:.1}%)", pct)
            }
        }
    }
}

/// Render the report as Markdown.
pub fn render_markdown(report: &ExportReport) -> String {
    let mut md = String::new();

    md.push_str(&format!("# Coverage Report — {}\n\n", report.project_name));

    // Snapshot info
    md.push_str(&format!("**Date:** {}\n", report.current.date));
    if let Some(sha) = &report.current.commit_sha {
        md.push_str(&format!("**Commit:** `{}`\n", if sha.len() > 7 { &sha[..7] } else { sha }));
    }
    if let Some(branch) = &report.current.branch {
        md.push_str(&format!("**Branch:** {}\n", branch));
    }
    md.push_str(&format!("**Status:** {}\n", report.overview.status));

    if let Some(baseline) = &report.baseline {
        md.push_str(&format!("\n**Compared against:** {}", baseline.date));
        if let Some(sha) = &baseline.commit_sha {
            md.push_str(&format!(" (`{}`)", if sha.len() > 7 { &sha[..7] } else { sha }));
        }
        md.push('\n');
    }

    // Overall summary
    let o = &report.overview;
    md.push_str("\n## Overview\n\n");
    md.push_str(&format!("- **Line Coverage:** {}{}\n", fmt_pct(o.overall_line_rate), fmt_delta(o.overall_line_delta)));
    md.push_str(&format!("- **Branch Coverage:** {}{}\n", fmt_pct(o.overall_branch_rate), fmt_delta(o.overall_branch_delta)));
    md.push_str(&format!("- **Lines:** {} / {} covered\n", o.total_lines_covered, o.total_lines));
    md.push_str(&format!("- **Branches:** {} / {} covered\n", o.total_branches_covered, o.total_branches));
    md.push_str(&format!("- **Files:** {} total\n", o.total_files));

    // File distribution
    md.push_str("\n## File Distribution\n\n");
    md.push_str(&format!("| >= 80% (Passing) | < 50% (Failing) | 0% (No coverage) |\n"));
    md.push_str("|---|---|---|\n");
    md.push_str(&format!("| {} files | {} files | {} files |\n",
        o.files_above_80_pct,
        o.files_below_50_pct,
        o.files_with_zero_coverage,
    ));

    // Per-target breakdown
    if report.targets.len() > 1 {
        md.push_str("\n## Per-Target Breakdown\n\n");
    } else {
        md.push_str("\n## Target Details\n\n");
    }
    md.push_str("| Target | Lines | Branches | Covered / Total | Status |\n");
    md.push_str("|--------|-------|----------|-----------------|--------|\n");
    for t in &report.targets {
        md.push_str(&format!(
            "| {} | {}{} | {}{} | {} / {} | {} |\n",
            t.label,
            fmt_pct(t.line_rate),
            fmt_delta(t.line_delta),
            fmt_pct(t.branch_rate),
            fmt_delta(t.branch_delta),
            t.lines_covered,
            t.lines_total,
            t.status,
        ));
    }

    md.push_str("\n---\n*Generated by Omnivore*\n");
    md
}

/// Render the report as pretty-printed JSON.
pub fn render_json(report: &ExportReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}
