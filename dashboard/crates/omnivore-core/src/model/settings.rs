use serde::{Deserialize, Serialize};

/// Global settings for the Omnivore dashboard instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub default_line_threshold: f64,
    pub default_branch_threshold: f64,
    pub default_line_warn_threshold: f64,
    pub default_branch_warn_threshold: f64,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            default_line_threshold: 0.8,
            default_branch_threshold: 0.8,
            default_line_warn_threshold: 0.5,
            default_branch_warn_threshold: 0.5,
        }
    }
}
