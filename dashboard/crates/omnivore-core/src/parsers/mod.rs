pub mod lcov;
pub mod llvm_cov;
pub mod omnivore_json;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid lcov: {0}")]
    Lcov(String),
    #[error("Invalid llvm-cov export: {0}")]
    LlvmCov(String),
    #[error("Unknown format")]
    UnknownFormat,
}

/// Supported coverage report formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageFormat {
    Omnivore,
    Lcov,
    LlvmCov,
}

impl CoverageFormat {
    /// Parse a format string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "omnivore" => Some(Self::Omnivore),
            "lcov" => Some(Self::Lcov),
            "llvm-cov" | "llvm_cov" | "llvmcov" => Some(Self::LlvmCov),
            _ => None,
        }
    }

    /// Auto-detect format from content.
    pub fn detect(content: &str) -> Option<Self> {
        let trimmed = content.trim();
        if trimmed.starts_with('{') {
            // JSON — try to distinguish omnivore vs llvm-cov
            if trimmed.contains("\"format\"") && trimmed.contains("\"omnivore\"") {
                Some(Self::Omnivore)
            } else if trimmed.contains("\"type\"") && trimmed.contains("llvm.coverage") {
                Some(Self::LlvmCov)
            } else {
                // Default JSON to omnivore
                Some(Self::Omnivore)
            }
        } else if trimmed.starts_with("TN:") || trimmed.starts_with("SF:") {
            Some(Self::Lcov)
        } else {
            None
        }
    }
}
