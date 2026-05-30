use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::metrics::numeric_result_keys;

use super::config::DisqualifierRule;
use super::error::{ScoreError, Result};

/// TOML-safe key for a metric disqualifier field (e.g. `balance_drawdown_maximal_%` → `balance_drawdown_maximal_percent`).
pub fn metric_disqualify_key(metric: &str) -> String {
    let mut s = metric
        .replace(" (of total)", "_of_total")
        .replace('(', "")
        .replace(')', "")
        .replace(", ", "_")
        .replace(' ', "_")
        .replace("_%", "_percent")
        .replace('%', "percent");
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    s.to_ascii_lowercase()
}

/// All numeric metrics eligible for `[disqualifiers]` (excludes holding-time strings).
pub fn numeric_disqualify_metrics() -> Vec<&'static str> {
    numeric_result_keys().collect()
}

fn disqualify_key_lookup() -> HashMap<String, &'static str> {
    numeric_disqualify_metrics()
        .into_iter()
        .map(|m| (metric_disqualify_key(m), m))
        .collect()
}

/// Parse `[disqualifiers]` table keys like `profit_factor_below = 0.8`.
pub fn parse_disqualifiers(raw: &BTreeMap<String, toml::Value>) -> Result<Vec<DisqualifierRule>> {
    let lookup = disqualify_key_lookup();
    let mut rules = Vec::new();

    for (key, value) in raw {
        let threshold = value
            .as_float()
            .or_else(|| value.as_integer().map(|i| i as f64))
            .ok_or_else(|| {
                ScoreError::InvalidToml(format!(
                    "disqualifier \"{key}\" must be a number, got {value}"
                ))
            })?;

        let (toml_metric, op) = if let Some(stripped) = key.strip_suffix("_below") {
            (stripped, DisqualifierOp::Below)
        } else if let Some(stripped) = key.strip_suffix("_above") {
            (stripped, DisqualifierOp::Above)
        } else {
            return Err(ScoreError::InvalidToml(format!(
                "disqualifier \"{key}\": key must end with _below or _above"
            )));
        };

        let metric = lookup.get(toml_metric).copied().ok_or_else(|| {
            ScoreError::InvalidToml(format!(
                "unknown disqualifier metric \"{toml_metric}\" in \"{key}\"\n      Use {{metric}}_below or {{metric}}_above where metric is the TOML-safe name (e.g. balance_drawdown_maximal_percent, profit_factor)"
            ))
        })?;

        rules.push(DisqualifierRule {
            metric: metric.to_string(),
            op,
            threshold,
        });
    }

    Ok(rules)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisqualifierOp {
    Below,
    Above,
}

/// Returns violation messages; empty means all rules passed.
pub fn check_disqualifiers(
    rules: &[DisqualifierRule],
    results: &BTreeMap<String, Value>,
) -> Result<Vec<String>> {
    let mut violations = Vec::new();

    for rule in rules {
        let value = extract_numeric(results, &rule.metric)?;
        let failed = match rule.op {
            DisqualifierOp::Below => value < rule.threshold,
            DisqualifierOp::Above => value > rule.threshold,
        };
        if failed {
            let op_label = match rule.op {
                DisqualifierOp::Below => "below",
                DisqualifierOp::Above => "above",
            };
            violations.push(format!(
                "{} {op_label} {} (value: {value})",
                rule.metric, rule.threshold
            ));
        }
    }

    Ok(violations)
}

fn extract_numeric(results: &BTreeMap<String, Value>, key: &str) -> Result<f64> {
    let value = results.get(key).ok_or_else(|| {
        ScoreError::CalculationFailed(format!("metric \"{key}\" not found in results"))
    })?;
    match value {
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(f64::NAN);
            if !f.is_finite() {
                return Err(ScoreError::CalculationFailed(format!(
                    "metric \"{key}\" is not finite"
                )));
            }
            Ok(f)
        }
        _ => Err(ScoreError::CalculationFailed(format!(
            "metric \"{key}\" is not numeric"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn metric_disqualify_key_maps_percent_and_parens() {
        assert_eq!(
            metric_disqualify_key("balance_drawdown_maximal_%"),
            "balance_drawdown_maximal_percent"
        );
        assert_eq!(
            metric_disqualify_key("profit_trades_% (of total)"),
            "profit_trades_percent_of_total"
        );
        assert_eq!(
            metric_disqualify_key("correlation (Profits, MFE)"),
            "correlation_profits_mfe"
        );
    }

    #[test]
    fn parse_disqualifier_rules() {
        let mut raw = BTreeMap::new();
        raw.insert(
            "profit_factor_below".into(),
            toml::Value::Float(0.8),
        );
        raw.insert(
            "balance_drawdown_maximal_percent_above".into(),
            toml::Value::Float(40.0),
        );
        let rules = parse_disqualifiers(&raw).unwrap();
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().any(|r| r.metric == "profit_factor" && r.threshold == 0.8));
        assert!(rules
            .iter()
            .any(|r| r.metric == "balance_drawdown_maximal_%" && r.threshold == 40.0));
    }

    #[test]
    fn check_below_and_above() {
        let rules = vec![
            DisqualifierRule {
                metric: "profit_factor".into(),
                op: DisqualifierOp::Below,
                threshold: 0.8,
            },
            DisqualifierRule {
                metric: "total_net_profit".into(),
                op: DisqualifierOp::Below,
                threshold: 0.0,
            },
        ];
        let mut results = BTreeMap::new();
        results.insert("profit_factor".into(), json!(0.5));
        results.insert("total_net_profit".into(), json!(-100.0));
        let v = check_disqualifiers(&rules, &results).unwrap();
        assert_eq!(v.len(), 2);
    }
}
