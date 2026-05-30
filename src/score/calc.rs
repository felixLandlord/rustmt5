use std::collections::BTreeMap;

use serde_json::Value;

use super::config::{effective_decay, MetricConfig, ScoreConfigFile};
use super::error::{ScoreError, Result};
use super::normalize::normalize_metric;

#[derive(Debug, Clone)]
pub struct MetricBreakdown {
    pub name: String,
    pub weight: f64,
    #[allow(dead_code)]
    pub raw_value: f64,
    pub normalized: f64,
    pub contribution: f64,
}

#[derive(Debug, Clone)]
pub struct ScoreResult {
    pub report_id: u32,
    pub final_score: f64,
    pub passed: bool,
    pub breakdown: Vec<MetricBreakdown>,
}

pub fn score_report(
    config: &ScoreConfigFile,
    report_id: u32,
    results: &BTreeMap<String, Value>,
    pass_threshold: f64,
) -> Result<ScoreResult> {
    let mut items = Vec::new();
    for m in &config.metrics {
        let raw = extract_numeric(results, &m.name)?;
        let normalized = normalize_metric(m, raw);
        items.push((m.clone(), raw, normalized));
    }

    let method = config.scoring.method.as_str();
    let (final_score, breakdown) = match method {
        "weighted_sum" | "weighted_average" => weighted_average(&items),
        "geometric_mean" => geometric_mean(&items),
        "harmonic_mean" => harmonic_mean(&items),
        "exponential_weighted" => {
            exponential_weighted(&items, effective_decay(config))
        }
        other => {
            return Err(ScoreError::CalculationFailed(format!(
                "unsupported method: {other}"
            )));
        }
    };

    Ok(ScoreResult {
        report_id,
        final_score,
        passed: final_score >= pass_threshold,
        breakdown,
    })
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

fn build_breakdown(
    items: &[(MetricConfig, f64, f64)],
    contributions: &[f64],
) -> Vec<MetricBreakdown> {
    items
        .iter()
        .zip(contributions.iter())
        .map(|((m, raw, norm), contrib)| MetricBreakdown {
            name: m.name.clone(),
            weight: m.weight,
            raw_value: *raw,
            normalized: *norm,
            contribution: *contrib,
        })
        .collect()
}

fn weighted_average(items: &[(MetricConfig, f64, f64)]) -> (f64, Vec<MetricBreakdown>) {
    let weight_sum: f64 = items.iter().map(|(m, _, _)| m.weight).sum();
    if weight_sum == 0.0 {
        return (0.0, build_breakdown(items, &vec![0.0; items.len()]));
    }
    let contribs: Vec<f64> = items
        .iter()
        .map(|(m, _, norm)| norm * m.weight / weight_sum)
        .collect();
    let score: f64 = contribs.iter().sum();
    (score, build_breakdown(items, &contribs))
}

fn geometric_mean(items: &[(MetricConfig, f64, f64)]) -> (f64, Vec<MetricBreakdown>) {
    let weight_sum: f64 = items.iter().map(|(m, _, _)| m.weight).sum();
    if weight_sum == 0.0 {
        return (0.0, build_breakdown(items, &vec![0.0; items.len()]));
    }
    let mut product = 1.0;
    let contribs: Vec<f64> = items
        .iter()
        .map(|(m, _, norm)| {
            let fraction = (norm / 100.0).max(1e-9);
            let w = m.weight / weight_sum;
            product *= fraction.powf(w);
            fraction * 100.0 * w
        })
        .collect();
    (product * 100.0, build_breakdown(items, &contribs))
}

fn harmonic_mean(items: &[(MetricConfig, f64, f64)]) -> (f64, Vec<MetricBreakdown>) {
    let weight_sum: f64 = items.iter().map(|(m, _, _)| m.weight).sum();
    if weight_sum == 0.0 {
        return (0.0, build_breakdown(items, &vec![0.0; items.len()]));
    }
    let mut denom = 0.0;
    let contribs: Vec<f64> = items
        .iter()
        .map(|(m, _, norm)| {
            let fraction = (norm / 100.0).max(1e-9);
            denom += m.weight / fraction;
            0.0
        })
        .collect();
    let score = if denom == 0.0 {
        0.0
    } else {
        100.0 * weight_sum / denom
    };
    (score, build_breakdown(items, &contribs))
}

fn exponential_weighted(
    items: &[(MetricConfig, f64, f64)],
    decay: f64,
) -> (f64, Vec<MetricBreakdown>) {
    let exponent = 2.0 - decay;
    let weight_sum: f64 = items.iter().map(|(m, _, _)| m.weight).sum();
    if weight_sum == 0.0 {
        return (0.0, build_breakdown(items, &vec![0.0; items.len()]));
    }
    let contribs: Vec<f64> = items
        .iter()
        .map(|(m, _, norm)| {
            let fraction = (norm / 100.0).max(0.0);
            fraction.powf(exponent) * m.weight / weight_sum * 100.0
        })
        .collect();
    let score: f64 = contribs.iter().sum();
    (score, build_breakdown(items, &contribs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::config::{MetricConfig, ScoreConfigFile, ScoringSection};
    use serde_json::json;

    fn metric(name: &str, weight: f64, direction: &str) -> MetricConfig {
        MetricConfig {
            name: name.into(),
            weight,
            direction: direction.into(),
            min_value: None,
            cap_value: None,
        }
    }

    fn config(method: &str, metrics: Vec<MetricConfig>) -> ScoreConfigFile {
        ScoreConfigFile {
            scoring: ScoringSection {
                method: method.into(),
                pass_threshold: Some(50.0),
                decay: Some(1.0),
            },
            metrics,
        }
    }

    #[test]
    fn weighted_average_produces_finite_score() {
        let mut results = BTreeMap::new();
        results.insert("profit_factor".into(), json!(1.5));
        results.insert("sharpe_ratio".into(), json!(1.2));
        let cfg = config(
            "weighted_average",
            vec![
                metric("profit_factor", 50.0, "higher_is_better"),
                metric("sharpe_ratio", 50.0, "higher_is_better"),
            ],
        );
        let r = score_report(&cfg, 1, &results, 50.0).unwrap();
        assert!(r.final_score > 0.0 && r.final_score <= 100.0);
        assert_eq!(r.breakdown.len(), 2);
    }

    #[test]
    fn geometric_mean_penalizes_weak_link() {
        let mut results = BTreeMap::new();
        results.insert("profit_factor".into(), json!(3.0));
        results.insert("sharpe_ratio".into(), json!(0.1));
        let metrics = vec![
            metric("profit_factor", 50.0, "higher_is_better"),
            metric("sharpe_ratio", 50.0, "higher_is_better"),
        ];
        let avg = score_report(&config("weighted_average", metrics.clone()), 1, &results, 0.0)
            .unwrap()
            .final_score;
        let geo = score_report(&config("geometric_mean", metrics), 1, &results, 0.0)
            .unwrap()
            .final_score;
        assert!(geo < avg);
    }
}
