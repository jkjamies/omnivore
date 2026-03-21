pub mod go_coverprofile;
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
    #[error("Unknown format")]
    UnknownFormat,
}

/// Supported coverage report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    Omnivore,
    Lcov,
    LlvmCov,
    GoCoverprofile,
    PythonCoverage,
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
            _ => None,
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
        } else {
            None
        }
    }
}
