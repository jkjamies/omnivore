//! JaCoCo-compatible XML coverage parser.
//!
//! JaCoCo's XML report format is also what Kover emits via the `koverXmlReport`
//! Gradle task — the two are byte-for-byte structurally identical — so this one
//! parser ingests both (and any other producer of JaCoCo XML). The caller
//! decides the recorded provenance (`source`) and the [`CoverageTarget`].
//!
//! ## Fidelity notes
//!
//! JaCoCo records *covered instructions* per line (`ci`), not an execution
//! count, so [`LineCoverage::hit_count`] carries `ci` as a covered-instruction
//! proxy: `> 0` means the line was covered, `0` means missed. Branch coverage is
//! taken from each line's `cb`/`mb` (covered/missed branches), which sum to
//! exactly JaCoCo's `BRANCH` counter. The XML carries no source text, so
//! `source_content` is always `None` (the dashboard fetches source on demand).
//!
//! ## Security
//!
//! `quick-xml` never resolves external DTDs or entities, so this is not exposed
//! to XXE. As defense in depth we also strip the XML declaration and DOCTYPE
//! before handing the document to the deserializer.

use crate::model::coverage::{
    CoverageSnapshot, CoverageSummary, CoverageTarget, FileCoverage, LineCoverage, OmnivoreReport,
    ProjectInfo,
};
use crate::parsers::{IngestMeta, ParseError};
use serde::Deserialize;

// --- JaCoCo XML schema (subset we care about) ---

#[derive(Debug, Deserialize)]
struct Report {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "package", default)]
    packages: Vec<Package>,
    #[serde(rename = "counter", default)]
    counters: Vec<Counter>,
}

#[derive(Debug, Deserialize)]
struct Package {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "sourcefile", default)]
    sourcefiles: Vec<SourceFile>,
}

#[derive(Debug, Deserialize)]
struct SourceFile {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "line", default)]
    lines: Vec<Line>,
}

#[derive(Debug, Deserialize)]
struct Line {
    /// Line number.
    #[serde(rename = "@nr")]
    nr: i32,
    /// Missed instructions.
    #[serde(rename = "@mi", default)]
    #[allow(dead_code)]
    mi: i64,
    /// Covered instructions.
    #[serde(rename = "@ci", default)]
    ci: i64,
    /// Missed branches.
    #[serde(rename = "@mb", default)]
    mb: i64,
    /// Covered branches.
    #[serde(rename = "@cb", default)]
    cb: i64,
}

#[derive(Debug, Deserialize)]
struct Counter {
    #[serde(rename = "@type")]
    counter_type: String,
    #[serde(rename = "@missed", default)]
    missed: i64,
    #[serde(rename = "@covered", default)]
    covered: i64,
}

/// Parse a JaCoCo/Kover XML report.
///
/// `target` and `source` are supplied by the caller: JaCoCo XML doesn't encode
/// which execution environment produced it, so the ingest endpoint defaults the
/// target to `JVM_UNIT` (overridable via `?target=`) and sets the source from
/// the format alias (`kover` or `jacoco`).
pub fn parse(
    xml: &str,
    meta: &IngestMeta,
    target: CoverageTarget,
    source: &str,
) -> Result<(OmnivoreReport, CoverageSnapshot), ParseError> {
    let document = strip_prolog(xml);
    let report: Report = quick_xml::de::from_str(document)
        .map_err(|e| ParseError::Jacoco(format!("Malformed XML: {e}")))?;

    let mut files: Vec<FileCoverage> = Vec::new();
    // Running per-line branch totals, used as a fallback when the report omits
    // its top-level BRANCH counter (JaCoCo/Kover may do so even when lines carry
    // cb/mb), so real branch coverage isn't dropped.
    let mut agg_branches_covered: i64 = 0;
    let mut agg_branches_total: i64 = 0;

    for package in &report.packages {
        for sf in &package.sourcefiles {
            let path = if package.name.is_empty() {
                sf.name.clone()
            } else {
                format!("{}/{}", package.name, sf.name)
            };

            let mut lines: Vec<LineCoverage> = Vec::with_capacity(sf.lines.len());
            let mut file_lines_covered: i64 = 0;
            let mut file_branches_covered: i64 = 0;
            let mut file_branches_total: i64 = 0;

            for line in &sf.lines {
                // ci = covered instructions; > 0 means the line executed.
                lines.push(LineCoverage {
                    line_number: line.nr,
                    hit_count: line.ci,
                });
                if line.ci > 0 {
                    file_lines_covered += 1;
                }
                file_branches_covered += line.cb;
                file_branches_total += line.cb + line.mb;
            }

            if lines.is_empty() {
                continue;
            }

            // Keep lines ordered by number — packages usually already are, but a
            // sorted list keeps the file view and gutter marks stable.
            lines.sort_by_key(|l| l.line_number);

            let lines_total = lines.len() as i64;
            let line_rate = ratio(file_lines_covered, lines_total);
            let branch_rate = ratio(file_branches_covered, file_branches_total);

            agg_branches_covered += file_branches_covered;
            agg_branches_total += file_branches_total;

            files.push(FileCoverage {
                path,
                line_rate,
                branch_rate,
                lines,
                source_content: None,
            });
        }
    }

    if files.is_empty() {
        return Err(ParseError::Jacoco(
            "No sourcefiles with line coverage found in report".into(),
        ));
    }

    // Prefer the report-level counters (authoritative) and fall back to summing
    // the per-file data when a counter is absent.
    let (lines_covered, lines_total) = match report.counter("LINE") {
        Some(c) => (c.covered, c.covered + c.missed),
        None => {
            let total: i64 = files.iter().map(|f| f.lines.len() as i64).sum();
            let covered: i64 = files
                .iter()
                .map(|f| f.lines.iter().filter(|l| l.hit_count > 0).count() as i64)
                .sum();
            (covered, total)
        }
    };

    let (branches_covered, branches_total) = match report.counter("BRANCH") {
        Some(c) => (c.covered, c.covered + c.missed),
        None => (agg_branches_covered, agg_branches_total),
    };

    let project_id = meta
        .project_id
        .clone()
        .unwrap_or_else(|| "jacoco-project".into());
    let project_name = meta.project_name.clone().unwrap_or_else(|| {
        if report.name.is_empty() {
            "JaCoCo import".into()
        } else {
            report.name.clone()
        }
    });

    let report_out = OmnivoreReport {
        version: "0.1.0".into(),
        format: "jacoco-xml".into(),
        dependencies: None,
        project: ProjectInfo {
            id: project_id,
            name: project_name,
            commit_sha: meta.commit_sha.clone(),
            branch: meta.branch.clone(),
            target,
            source: Some(source.to_string()),
        },
        coverage: CoverageSummary {
            line_rate: ratio(lines_covered, lines_total),
            branch_rate: ratio(branches_covered, branches_total),
            lines_covered,
            lines_total,
            branches_covered,
            branches_total,
        },
        files,
    };

    let snapshot = CoverageSnapshot::from_report(&report_out, Some(source));
    Ok((report_out, snapshot))
}

impl Report {
    fn counter(&self, ty: &str) -> Option<&Counter> {
        self.counters.iter().find(|c| c.counter_type == ty)
    }
}

fn ratio(covered: i64, total: i64) -> f64 {
    if total > 0 {
        covered as f64 / total as f64
    } else {
        0.0
    }
}

/// Strip a leading `<?xml … ?>` declaration and `<!DOCTYPE … >` so the document
/// handed to the deserializer starts at the root `<report>` element. This keeps
/// any DTD out of the parser entirely (JaCoCo's DOCTYPE has no internal subset,
/// so the first `>` terminates it).
fn strip_prolog(xml: &str) -> &str {
    let mut s = xml.trim_start();
    if s.starts_with("<?xml") {
        if let Some(end) = s.find("?>") {
            s = s[end + 2..].trim_start();
        }
    }
    if s.starts_with("<!DOCTYPE") {
        if let Some(end) = s.find('>') {
            s = s[end + 1..].trim_start();
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::coverage::source;

    const SAMPLE_JACOCO: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd">
<report name="kmp-test-rig">
  <sessioninfo id="host" start="1710000000000" dump="1710000001000"/>
  <package name="com/example/app">
    <class name="com/example/app/Calculator" sourcefilename="Calculator.kt">
      <method name="add" desc="(II)I" line="5">
        <counter type="INSTRUCTION" missed="0" covered="4"/>
        <counter type="LINE" missed="0" covered="1"/>
      </method>
      <counter type="INSTRUCTION" missed="3" covered="10"/>
      <counter type="LINE" missed="1" covered="2"/>
      <counter type="BRANCH" missed="1" covered="1"/>
    </class>
    <sourcefile name="Calculator.kt">
      <line nr="5" mi="0" ci="4" mb="0" cb="0"/>
      <line nr="6" mi="0" ci="6" mb="1" cb="1"/>
      <line nr="7" mi="3" ci="0" mb="0" cb="0"/>
      <counter type="INSTRUCTION" missed="3" covered="10"/>
      <counter type="LINE" missed="1" covered="2"/>
      <counter type="BRANCH" missed="1" covered="1"/>
    </sourcefile>
  </package>
  <counter type="INSTRUCTION" missed="3" covered="10"/>
  <counter type="LINE" missed="1" covered="2"/>
  <counter type="BRANCH" missed="1" covered="1"/>
  <counter type="METHOD" missed="0" covered="1"/>
  <counter type="CLASS" missed="0" covered="1"/>
</report>
"#;

    #[test]
    fn parse_jacoco_basic() {
        let meta = IngestMeta {
            project_id: Some("kmp-app".into()),
            project_name: Some("KMP App".into()),
            commit_sha: Some("abc123".into()),
            branch: Some("main".into()),
        };
        let (report, snapshot) =
            parse(SAMPLE_JACOCO, &meta, CoverageTarget::JvmUnit, source::KOVER).unwrap();

        assert_eq!(report.format, "jacoco-xml");
        assert_eq!(report.project.id, "kmp-app");
        assert_eq!(report.project.target, CoverageTarget::JvmUnit);
        assert_eq!(report.files.len(), 1);

        // Path is package + sourcefile name.
        assert_eq!(report.files[0].path, "com/example/app/Calculator.kt");
        // Three <line> entries; line 7 is missed (ci=0).
        assert_eq!(report.files[0].lines.len(), 3);
        assert!(report.files[0].lines.iter().any(|l| l.line_number == 7 && l.hit_count == 0));
        assert!(report.files[0].lines.iter().any(|l| l.line_number == 5 && l.hit_count == 4));

        // Report-level counters: 2/3 lines, 1/2 branches.
        assert_eq!(snapshot.lines_covered, 2);
        assert_eq!(snapshot.lines_total, 3);
        assert_eq!(snapshot.branches_covered, 1);
        assert_eq!(snapshot.branches_total, 2);
        assert_eq!(snapshot.file_count, 1);

        // Provenance and canonical target string.
        assert_eq!(snapshot.source, source::KOVER);
        assert_eq!(snapshot.target, "JVM_UNIT");
    }

    #[test]
    fn parse_jacoco_default_package_and_source() {
        // Empty package name → path is just the sourcefile name; jacoco source.
        let xml = r#"<report name="r">
          <package name="">
            <sourcefile name="Main.kt">
              <line nr="1" mi="0" ci="2" mb="0" cb="0"/>
            </sourcefile>
          </package>
        </report>"#;
        let meta = IngestMeta::default();
        let (report, snapshot) =
            parse(xml, &meta, CoverageTarget::JvmUnit, source::JACOCO).unwrap();
        assert_eq!(report.files[0].path, "Main.kt");
        assert_eq!(report.project.id, "jacoco-project");
        assert_eq!(snapshot.source, source::JACOCO);
    }

    #[test]
    fn parse_jacoco_computes_totals_without_report_counters() {
        // No report-level counters → summary summed from per-file lines.
        let xml = r#"<report name="r">
          <package name="p">
            <sourcefile name="A.kt">
              <line nr="1" mi="0" ci="1" mb="0" cb="0"/>
              <line nr="2" mi="1" ci="0" mb="0" cb="0"/>
            </sourcefile>
          </package>
        </report>"#;
        let meta = IngestMeta::default();
        let (_, snapshot) = parse(xml, &meta, CoverageTarget::JvmUnit, source::JACOCO).unwrap();
        assert_eq!(snapshot.lines_total, 2);
        assert_eq!(snapshot.lines_covered, 1);
    }

    #[test]
    fn parse_jacoco_branch_fallback_without_report_counter() {
        // No report-level BRANCH counter, but lines carry cb/mb — branch totals
        // must fall back to the per-line sums rather than reporting 0/0.
        let xml = r#"<report name="r">
          <package name="p">
            <sourcefile name="A.kt">
              <line nr="1" mi="0" ci="2" mb="1" cb="1"/>
              <line nr="2" mi="0" ci="2" mb="0" cb="2"/>
            </sourcefile>
          </package>
        </report>"#;
        let meta = IngestMeta::default();
        let (report, snapshot) =
            parse(xml, &meta, CoverageTarget::JvmUnit, source::JACOCO).unwrap();
        // covered = 1 + 2 = 3; total = (1+1) + (0+2) = 4.
        assert_eq!(snapshot.branches_covered, 3);
        assert_eq!(snapshot.branches_total, 4);
        assert!((report.coverage.branch_rate - 0.75).abs() < 1e-9);
    }

    #[test]
    fn parse_jacoco_empty_fails() {
        let xml = r#"<report name="r"></report>"#;
        let meta = IngestMeta::default();
        assert!(parse(xml, &meta, CoverageTarget::JvmUnit, source::JACOCO).is_err());
    }

    #[test]
    fn parse_jacoco_target_override() {
        let xml = r#"<report name="r">
          <package name="p">
            <sourcefile name="A.kt"><line nr="1" mi="0" ci="1" mb="0" cb="0"/></sourcefile>
          </package>
        </report>"#;
        let meta = IngestMeta::default();
        let (_, snapshot) =
            parse(xml, &meta, CoverageTarget::AndroidInstrumented, source::JACOCO).unwrap();
        assert_eq!(snapshot.target, "ANDROID_INSTRUMENTED");
    }
}
