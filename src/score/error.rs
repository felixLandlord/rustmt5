#[derive(Debug, thiserror::Error)]
pub enum ScoreError {
    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("invalid JSON: {0}")]
    InvalidJson(String),

    #[error("invalid TOML: {0}")]
    InvalidToml(String),

    #[error("✗ Cannot calculate score\n\n{details}")]
    CombinedFailure { details: String },

    #[error("✗ Invalid score configuration: {path}\n\n  Config errors:\n{details}")]
    ConfigValidationFailed { path: String, details: String },

    #[error("✗ Invalid metrics file: {path}\n\n  Metrics errors:\n{details}")]
    MetricsValidationFailed { path: String, details: String },

    #[error("calculation failed: {0}")]
    CalculationFailed(String),
}

pub type Result<T> = std::result::Result<T, ScoreError>;

impl ScoreError {
    pub fn config_errors(path: &str, lines: Vec<String>) -> Self {
        let details = lines
            .into_iter()
            .map(|l| format!("    - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        Self::ConfigValidationFailed {
            path: path.to_string(),
            details,
        }
    }

    pub fn metrics_errors(path: &str, lines: Vec<String>) -> Self {
        let details = lines
            .into_iter()
            .map(|l| format!("    - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        Self::MetricsValidationFailed {
            path: path.to_string(),
            details,
        }
    }

    pub fn combined(config_path: &str, config_lines: Vec<String>, metrics_path: &str, metrics_lines: Vec<String>) -> Self {
        let mut parts = Vec::new();
        if !config_lines.is_empty() {
            let details = config_lines
                .into_iter()
                .map(|l| format!("    - {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("  Config errors ({config_path}):\n{details}"));
        }
        if !metrics_lines.is_empty() {
            let details = metrics_lines
                .into_iter()
                .map(|l| format!("    - {l}"))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("  Metrics errors ({metrics_path}):\n{details}"));
        }
        Self::CombinedFailure {
            details: parts.join("\n\n"),
        }
    }
}

pub(crate) fn rel_path(path: &std::path::Path) -> String {
    path.strip_prefix(std::env::current_dir().unwrap_or_default())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}
