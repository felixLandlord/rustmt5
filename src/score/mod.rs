mod calc;
mod config;
mod disqualify;
mod error;
mod normalize;

pub use error::ScoreError;

use std::path::Path;

use error::{rel_path, Result};

use crate::metrics::{validate_metrics_file, MetricsFile};

pub fn run(config_path: &Path, metrics_path: &Path) -> Result<()> {
    let config_rel = rel_path(config_path);
    let metrics_rel = rel_path(metrics_path);

    let mut config_errors = Vec::new();
    let mut metrics_errors = Vec::new();

    let config = match config::load_config(config_path) {
        Ok(c) => c,
        Err(e) => return Err(e),
    };

    if let Err(e) = config::validate_config(&config, config_path) {
        if let ScoreError::ConfigValidationFailed { details, .. } = e {
            config_errors = parse_error_lines(&details);
        } else {
            return Err(e);
        }
    }

    let metrics_file = match load_metrics(metrics_path) {
        Ok(m) => m,
        Err(ScoreError::MetricsValidationFailed { details, .. }) => {
            metrics_errors = parse_error_lines(&details);
            if config_errors.is_empty() {
                return Err(ScoreError::metrics_errors(&metrics_rel, metrics_errors));
            }
            return Err(ScoreError::combined(
                &config_rel,
                config_errors,
                &metrics_rel,
                metrics_errors,
            ));
        }
        Err(e) => return Err(e),
    };

    if !config_errors.is_empty() || !metrics_errors.is_empty() {
        return Err(ScoreError::combined(
            &config_rel,
            config_errors,
            &metrics_rel,
            metrics_errors,
        ));
    }

    let threshold = config::pass_threshold(&config);
    let mut results = Vec::new();
    for report in &metrics_file.reports {
        results.push(calc::score_report(
            &config,
            report.id,
            &report.results,
            threshold,
        )?);
    }

    print_results(&config_rel, &metrics_rel, &config, threshold, &results);
    Ok(())
}

fn load_metrics(path: &Path) -> Result<MetricsFile> {
    if !path.exists() {
        return Err(ScoreError::FileNotFound(rel_path(path)));
    }
    let content = std::fs::read_to_string(path).map_err(|e| ScoreError::InvalidJson(e.to_string()))?;
    let file: MetricsFile =
        serde_json::from_str(&content).map_err(|e| ScoreError::InvalidJson(e.to_string()))?;
    validate_metrics_file(&file, &rel_path(path)).map_err(|e| match e {
        crate::metrics::MetricsError::ValidationFailed { details, .. } => {
            ScoreError::metrics_errors(&rel_path(path), parse_error_lines(&details))
        }
        crate::metrics::MetricsError::FileNotFound(p) => ScoreError::FileNotFound(p),
        crate::metrics::MetricsError::InvalidJson(m) => ScoreError::InvalidJson(m),
        other => ScoreError::InvalidJson(other.to_string()),
    })?;
    Ok(file)
}

fn parse_error_lines(details: &str) -> Vec<String> {
    details
        .lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .map(|l| l.trim_start_matches("- ").to_string())
        .collect()
}

fn print_results(
    config_rel: &str,
    metrics_rel: &str,
    config: &config::ScoreConfigFile,
    threshold: f64,
    results: &[calc::ScoreResult],
) {
    let method = &config.scoring.method;
    let weight_sum: f64 = config.metrics.iter().map(|m| m.weight).sum();

    if results.len() == 1 {
        let r = &results[0];
        println!("✓ Score calculated successfully");
        println!("  Config: {config_rel} (method: {method})");
        println!("  Metrics: {metrics_rel} (report ID: {})", r.report_id);
        println!();
        println!("  Results:");
        println!("    Report ID: {}", r.report_id);

        if r.disqualified {
            println!("    Status: FAIL (disqualified)");
            println!();
            println!("  Disqualifiers:");
            for v in &r.disqualifier_violations {
                println!("    - {v}");
            }
            println!();
            println!("  Status: FAIL (hard disqualifier triggered)");
            return;
        }

        println!("    Score: {:.1} / 100", r.final_score);
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("    Status: {status} (threshold: {threshold:.1})");
        println!();
        println!("  Breakdown ({method}):");
        for b in &r.breakdown {
            let pct = if weight_sum > 0.0 {
                (b.weight / weight_sum) * 100.0
            } else {
                0.0
            };
            println!(
                "    {} ({:.0}%):          {:.1} → {:.1}",
                b.name, pct, b.normalized, b.contribution
            );
        }
        println!("                                  ─────────────");
        println!("    Final Score:                  {:.1} / 100", r.final_score);
        println!();
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("  Status: {status} (≥ {threshold:.1})");
    } else {
        println!("✓ Score calculated for {} reports", results.len());
        println!("  Config: {config_rel} (method: {method})");
        println!("  Metrics: {metrics_rel}");
        println!();
        println!("  Summary:");
        let passed = results.iter().filter(|r| r.passed).count();
        for r in results {
            if r.disqualified {
                println!(
                    "    Report {}: DISQUALIFIED ({})",
                    r.report_id,
                    r.disqualifier_violations.join("; ")
                );
            } else {
                let status = if r.passed { "PASS" } else { "FAIL" };
                println!(
                    "    Report {}: {:.1} / 100 ({status})",
                    r.report_id, r.final_score
                );
            }
        }
        let rate = if results.is_empty() {
            0.0
        } else {
            (passed as f64 / results.len() as f64) * 100.0
        };
        println!();
        println!("  Pass Rate: {rate:.1}% ({passed}/{})", results.len());
    }
}
