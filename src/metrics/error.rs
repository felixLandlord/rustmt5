use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("file not found: {0}")]
    FileNotFound(String),

    #[error("invalid JSON: {0}")]
    InvalidJson(String),

    #[error("HTML parsing failed: {0}")]
    HtmlParsingFailed(String),

    #[error("{summary}\n  {section}:\n{details}")]
    ValidationFailed {
        summary: String,
        section: String,
        details: String,
    },
}

impl MetricsError {
    pub fn validation(report_name: &str, section: &str, lines: Vec<String>) -> Self {
        let details = lines
            .into_iter()
            .map(|l| format!("    - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        Self::ValidationFailed {
            summary: format!("✗ Invalid metrics in report: {report_name}"),
            section: section.to_string(),
            details,
        }
    }

    pub fn metrics_file_validation(file: &str, lines: Vec<String>) -> Self {
        let details = lines
            .into_iter()
            .map(|l| format!("    - {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        Self::ValidationFailed {
            summary: format!("✗ Invalid metrics file: {file}"),
            section: "Metrics errors".to_string(),
            details,
        }
    }
}

pub type Result<T> = std::result::Result<T, MetricsError>;

pub(crate) fn path_display(path: &std::path::Path) -> String {
    path.strip_prefix(std::env::current_dir().unwrap_or_default())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub(crate) fn path_buf_display(path: &PathBuf) -> String {
    path_display(path.as_path())
}
