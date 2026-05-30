use std::collections::BTreeMap;

use serde_json::Value;

use super::error::{MetricsError, Result};
use super::schema::{self, REQUIRED_RESULT_KEYS};
use super::types::{MetricsFile, ReportEntry};

pub fn validate_extracted_report(report: &ReportEntry, report_name: &str) -> Result<()> {
    validate_settings(&report.settings, report_name)?;
    let errors = validate_results_map(&report.results, None);
    if !errors.is_empty() {
        return Err(MetricsError::validation(
            report_name,
            "Results section",
            errors,
        ));
    }
    Ok(())
}

pub fn validate_settings(
    settings: &super::types::ReportSettings,
    report_name: &str,
) -> Result<()> {
    let mut errors = Vec::new();
    if settings.expert.is_empty() {
        errors.push("Missing key: \"expert\"".into());
    }
    if settings.symbol.is_empty() {
        errors.push("Missing key: \"symbol\"".into());
    }
    if settings.period.timeframe.is_empty() {
        errors.push("Missing key: \"period.timeframe\"".into());
    }
    if settings.period.from_date.is_empty() {
        errors.push("Missing key: \"period.from_date\"".into());
    }
    if settings.period.to_date.is_empty() {
        errors.push("Missing key: \"period.to_date\"".into());
    }
    if settings.currency.is_empty() {
        errors.push("Missing key: \"currency\"".into());
    }
    if settings.leverage.is_empty() {
        errors.push("Missing key: \"leverage\"".into());
    }
    if !errors.is_empty() {
        return Err(MetricsError::validation(
            report_name,
            "Settings section",
            errors,
        ));
    }
    Ok(())
}

pub fn validate_results_map(results: &BTreeMap<String, Value>, report_id: Option<u32>) -> Vec<String> {
    let mut errors = Vec::new();
    let prefix = report_id
        .map(|id| format!("Report {id}: "))
        .unwrap_or_default();

    for key in REQUIRED_RESULT_KEYS {
        if !results.contains_key(*key) {
            errors.push(format!("{prefix}Missing key: \"{key}\""));
        }
    }

    for (key, value) in results {
        if let Some(err) = validate_result_value(key, value) {
            errors.push(format!("{prefix}{err}"));
        }
    }

    errors
}

pub fn validate_result_value(key: &str, value: &Value) -> Option<String> {
    if value.is_null() {
        return Some(format!("Invalid value: \"{key}\" = null\n      Cannot process null values"));
    }

    if schema::is_time_key(key) {
        if !value.is_string() {
            return Some(type_error(key, "string", value));
        }
        return None;
    }

    if schema::is_integer_key(key) {
        match value {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    if !f.is_finite() {
                        return Some(invalid_numeric(key, value));
                    }
                }
            }
            _ => return Some(type_error(key, "integer", value)),
        }
        return None;
    }

    match value {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(f64::NAN);
            if !f.is_finite() {
                return Some(invalid_numeric(key, value));
            }
        }
        Value::String(s) => {
            if s.eq_ignore_ascii_case("n/a") || s.eq_ignore_ascii_case("nan") {
                return Some(invalid_numeric(key, value));
            }
            return Some(type_error(key, "float", value));
        }
        _ => return Some(type_error(key, "float", value)),
    }

    None
}

fn type_error(key: &str, expected: &str, value: &Value) -> String {
    format!(
        "Type error: \"{key}\"\n      Expected: {expected}\n      Got: {}",
        value_display(value)
    )
}

fn invalid_numeric(key: &str, value: &Value) -> String {
    format!(
        "Invalid value: \"{key}\" = {}\n      Expected: numeric (float)\n      Cannot process NaN values",
        value_display(value)
    )
}

fn value_display(value: &Value) -> String {
    match value {
        Value::String(s) => format!("string \"{s}\""),
        Value::Number(n) => n.to_string(),
        _ => format!("{value}"),
    }
}

pub fn validate_metrics_file(file: &MetricsFile, file_label: &str) -> Result<()> {
    let mut all_errors = Vec::new();
    if file.reports.is_empty() {
        all_errors.push("No reports in file".into());
    }
    for report in &file.reports {
        let id = report.id;
        for e in validate_settings_errors(&report.settings) {
            all_errors.push(format!("Report {id}: {e}"));
        }
        all_errors.extend(validate_results_map(&report.results, Some(id)));
    }
    if !all_errors.is_empty() {
        return Err(MetricsError::metrics_file_validation(file_label, all_errors));
    }
    Ok(())
}

fn validate_settings_errors(settings: &super::types::ReportSettings) -> Vec<String> {
    let mut errors = Vec::new();
    if settings.expert.is_empty() {
        errors.push("Missing key: \"expert\"".into());
    }
    if settings.symbol.is_empty() {
        errors.push("Missing key: \"symbol\"".into());
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejects_missing_result_keys() {
        let mut results = BTreeMap::new();
        results.insert("profit_factor".into(), json!(1.0));
        let errors = validate_results_map(&results, None);
        assert!(errors.iter().any(|e| e.contains("Missing key")));
    }

    #[test]
    fn rejects_nan_values() {
        let mut results = BTreeMap::new();
        for key in REQUIRED_RESULT_KEYS {
            results.insert((*key).into(), json!(1));
        }
        results.insert("sharpe_ratio".into(), json!(f64::NAN));
        let errors = validate_results_map(&results, None);
        assert!(errors.iter().any(|e| e.contains("sharpe_ratio")));
    }
}

pub fn count_valid_metrics(results: &BTreeMap<String, Value>) -> usize {
    REQUIRED_RESULT_KEYS
        .iter()
        .filter(|k| {
            results
                .get(**k)
                .is_some_and(|v| validate_result_value(k, v).is_none())
        })
        .count()
}
