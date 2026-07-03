pub mod go_coverprofile;
pub mod jacoco_xml;
pub mod lcov;
pub mod llvm_cov;
pub mod omnivore_json;
pub mod python_coverage;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid lcov: {0}")]
    Lcov(String),
    #[error("Invalid llvm-cov export: {0}")]
    LlvmCov(String),
    #[error("Invalid Go coverprofile: {0}")]
    GoCoverprofile(String),
    #[error("Invalid Python coverage.py JSON: {0}")]
    PythonCoverage(String),
    #[error("Invalid JaCoCo/Kover XML: {0}")]
    Jacoco(String),
    #[error("Unknown format")]
    UnknownFormat,
}

/// Metadata that formats without embedded project info (lcov, llvm-cov, Go,
/// Python, JaCoCo/Kover XML) need supplied externally — mapped from the ingest
/// endpoint's query parameters. Shared by every parser so adding a format never
/// means redeclaring the same struct.
#[derive(Debug, Clone, Default)]
pub struct IngestMeta {
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub commit_sha: Option<String>,
    pub branch: Option<String>,
}

/// Supported coverage report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    Omnivore,
    Lcov,
    LlvmCov,
    GoCoverprofile,
    PythonCoverage,
    /// JaCoCo-compatible XML — produced by JaCoCo directly and by Kover's
    /// `koverXmlReport` task. The `Jacoco`/`Kover` variants share one parser and
    /// differ only in the recorded provenance (`source`).
    Jacoco,
    Kover,
}

impl CoverageFormat {
    /// Parse a format string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "omnivore" => Some(Self::Omnivore),
            "lcov" => Some(Self::Lcov),
            "llvm-cov" | "llvm_cov" | "llvmcov" => Some(Self::LlvmCov),
            "go" | "go-coverprofile" | "go_coverprofile" | "coverprofile" => Some(Self::GoCoverprofile),
            "python" | "python-coverage" | "python_coverage" | "coveragepy" | "coverage.py" => Some(Self::PythonCoverage),
            "kover" => Some(Self::Kover),
            "jacoco" | "jacoco-xml" | "jacoco_xml" => Some(Self::Jacoco),
            _ => None,
        }
    }

    /// Canonical provenance (`source`) recorded for reports of this format.
    pub fn source(&self) -> &'static str {
        use crate::model::coverage::source;
        match self {
            Self::Omnivore => source::OMNIVORE_AGENT,
            Self::Lcov => source::LCOV,
            Self::LlvmCov => source::LLVM_COV,
            Self::GoCoverprofile => source::GO,
            Self::PythonCoverage => source::PYTHON_COVERAGE,
            Self::Jacoco => source::JACOCO,
            Self::Kover => source::KOVER,
        }
    }

    /// Auto-detect format from content.
    pub fn detect(content: &str) -> Option<Self> {
        let trimmed = content.trim();
        if trimmed.starts_with('{') {
            // JSON — distinguish between formats by content
            if trimmed.contains("\"format\"") && trimmed.contains("\"omnivore\"") {
                Some(Self::Omnivore)
            } else if trimmed.contains("\"type\"") && trimmed.contains("llvm.coverage") {
                Some(Self::LlvmCov)
            } else if trimmed.contains("\"executed_lines\"") && trimmed.contains("\"num_statements\"") {
                Some(Self::PythonCoverage)
            } else {
                // Default JSON to omnivore
                Some(Self::Omnivore)
            }
        } else if trimmed.starts_with("TN:") || trimmed.starts_with("SF:") {
            Some(Self::Lcov)
        } else if trimmed.starts_with("mode:") {
            Some(Self::GoCoverprofile)
        } else if is_jacoco_xml(trimmed) {
            // JaCoCo and Kover XML are structurally identical, so detection can't
            // tell them apart — default provenance to plain JaCoCo. Callers who
            // want the report attributed to Kover pass `?format=kover` explicitly.
            Some(Self::Jacoco)
        } else {
            None
        }
    }
}

/// Recognize a JaCoCo-compatible XML report (as emitted by JaCoCo and Kover's
/// `koverXmlReport`). Matches the JaCoCo DTD public identifier or a `<report>`
/// root element, tolerating a leading XML declaration and/or DOCTYPE.
fn is_jacoco_xml(trimmed: &str) -> bool {
    if !trimmed.starts_with('<') {
        return false;
    }
    // Only sniff the head of the document — the report/DTD markers appear early.
    // Walk to a char boundary at/under 512 bytes so slicing never splits UTF-8.
    let mut head_len = trimmed.len().min(512);
    while head_len > 0 && !trimmed.is_char_boundary(head_len) {
        head_len -= 1;
    }
    let head = &trimmed[..head_len];
    head.contains("//JACOCO//DTD") || head.contains("<report")
}
