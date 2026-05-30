mod error;
mod io;
mod parse;
mod schema;
mod types;
mod validate;

pub use error::MetricsError;
pub use schema::ALLOWED_SCORE_METRICS;
pub use types::MetricsFile;
pub use validate::validate_metrics_file;

use std::path::{Path, PathBuf};

use error::{path_buf_display, Result};
use parse::{parse_html_report, report_stem};
use validate::{count_valid_metrics, validate_extracted_report};

/// Run the `metrics` subcommand.
pub fn run(
    report_path: &Path,
    output: Option<PathBuf>,
    append: Option<&Path>,
) -> Result<()> {
    if !report_path.exists() {
        return Err(MetricsError::FileNotFound(path_buf_display(
            &report_path.to_path_buf(),
        )));
    }
    if report_path.extension().and_then(|e| e.to_str()) != Some("htm")
        && report_path.extension().and_then(|e| e.to_str()) != Some("html")
    {
        return Err(MetricsError::HtmlParsingFailed(
            "expected .htm or .html report file".into(),
        ));
    }

    let html = crate::text_decode::read_text_file(report_path)
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;

    let stem = report_stem(report_path);
    let entry = parse_html_report(&html, &stem)?;
    validate_extracted_report(&entry, &stem)?;

    let valid_count = count_valid_metrics(&entry.results);
    let total = schema::metric_count();

    let (out_path, report_id, duplicate_of) =
        io::write_report(report_path, entry, output, append)?;

    print_success(&stem, valid_count, total, report_id, &duplicate_of, &out_path);
    Ok(())
}

fn print_success(
    report_name: &str,
    valid: usize,
    total: usize,
    id: u32,
    duplicate_of: &[u32],
    path: &Path,
) {
    let rel = path
        .strip_prefix(std::env::current_dir().unwrap_or_default())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    println!("✓ Extracted metrics from {report_name}.htm");
    println!("  Metrics: {valid} / {total} present and valid");
    if duplicate_of.is_empty() {
        println!("  Report ID: {id}");
    } else {
        let ids = duplicate_of
            .iter()
            .map(|i| format!("ID {i}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  Report ID: {id} - duplicate of [{ids}]");
    }
    println!("  Saved to: {rel}");
}
