use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::metrics::ALLOWED_SCORE_METRICS;

use super::disqualify::{self, DisqualifierOp};
use super::error::{rel_path, ScoreError, Result};

pub const SCORING_METHODS: &[&str] = &[
    "weighted_sum",
    "weighted_average",
    "geometric_mean",
    "harmonic_mean",
    "exponential_weighted",
];

#[derive(Debug, Clone, Deserialize)]
pub struct ScoreConfigFile {
    pub scoring: ScoringSection,
    #[serde(default)]
    pub disqualifiers: BTreeMap<String, toml::Value>,
    pub metrics: Vec<MetricConfig>,
    /// Parsed from `[disqualifiers]` after load (not in TOML directly).
    #[serde(skip)]
    pub disqualifier_rules: Vec<DisqualifierRule>,
}

#[derive(Debug, Clone)]
pub struct DisqualifierRule {
    pub metric: String,
    pub op: DisqualifierOp,
    pub threshold: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoringSection {
    pub method: String,
    #[serde(default)]
    pub pass_threshold: Option<f64>,
    #[serde(default)]
    pub decay: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricConfig {
    pub name: String,
    pub weight: f64,
    pub direction: String,
    #[serde(default)]
    pub min_value: Option<f64>,
    #[serde(default)]
    pub cap_value: Option<f64>,
}

pub fn load_config(path: &Path) -> Result<ScoreConfigFile> {
    if !path.exists() {
        return Err(ScoreError::FileNotFound(rel_path(path)));
    }
    let content = fs::read_to_string(path).map_err(|e| ScoreError::InvalidToml(e.to_string()))?;
    let mut config: ScoreConfigFile =
        toml::from_str(&content).map_err(|e| ScoreError::InvalidToml(e.to_string()))?;
    config.disqualifier_rules = disqualify::parse_disqualifiers(&config.disqualifiers)?;
    Ok(config)
}

pub fn validate_config(config: &ScoreConfigFile, path: &Path) -> Result<()> {
    let mut errors = Vec::new();
    let path_str = rel_path(path);

    if !SCORING_METHODS.contains(&config.scoring.method.as_str()) {
        errors.push(format!(
            "Unknown scoring method: \"{}\"\n      Allowed: {}",
            config.scoring.method,
            SCORING_METHODS.join(", ")
        ));
    }

    if config.metrics.is_empty() {
        errors.push("At least one [[metrics]] entry is required".into());
    }

    let mut weight_sum = 0.0;
    for m in &config.metrics {
        if !ALLOWED_SCORE_METRICS.contains(&m.name.as_str()) {
            errors.push(format!(
                "Unknown metric: \"{}\"\n      Allowed: profit_factor, sharpe_ratio, recovery_factor, ...",
                m.name
            ));
        }
        if m.weight < 0.0 {
            errors.push(format!(
                "Invalid weight: \"{}\"\n      Weight must be >= 0, got: {}",
                m.name, m.weight
            ));
        }
        weight_sum += m.weight;

        if m.direction != "higher_is_better" && m.direction != "lower_is_better" {
            errors.push(format!(
                "Invalid direction for \"{}\": \"{}\"\n      Use \"higher_is_better\" or \"lower_is_better\"",
                m.name, m.direction
            ));
        }

        if let (Some(min), Some(cap)) = (m.min_value, m.cap_value) {
            if min >= cap {
                errors.push(format!(
                    "min_value must be < cap_value for \"{}\" (min={min}, cap={cap})",
                    m.name
                ));
            }
        }
    }

    if matches!(
        config.scoring.method.as_str(),
        "weighted_sum" | "weighted_average"
    ) {
        let diff = (weight_sum - 100.0).abs();
        if diff > 1.0 {
            errors.push(format!(
                "Weight sum: {weight_sum:.1}\n      Expected: 100.0 (or 0 if weights are relative)"
            ));
        }
    }

    if let Some(threshold) = config.scoring.pass_threshold {
        if !(0.0..=100.0).contains(&threshold) {
            errors.push(format!(
                "pass_threshold must be between 0 and 100, got: {threshold}"
            ));
        }
    }

    if config.scoring.method == "exponential_weighted" {
        let decay = config.scoring.decay.unwrap_or(1.0);
        if decay <= 0.0 || decay >= 2.0 {
            errors.push(format!(
                "Invalid decay: {decay}\n      Must be between 0 and 2 (exclusive)"
            ));
        }
    }

    if !errors.is_empty() {
        return Err(ScoreError::config_errors(&path_str, errors));
    }
    Ok(())
}

pub fn effective_decay(config: &ScoreConfigFile) -> f64 {
    config.scoring.decay.unwrap_or(1.0)
}

pub fn pass_threshold(config: &ScoreConfigFile) -> f64 {
    config.scoring.pass_threshold.unwrap_or(60.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn rejects_unknown_scoring_method() {
        let cfg = ScoreConfigFile {
            scoring: ScoringSection {
                method: "invalid".into(),
                pass_threshold: Some(60.0),
                decay: None,
            },
            disqualifiers: BTreeMap::new(),
            disqualifier_rules: vec![],
            metrics: vec![MetricConfig {
                name: "profit_factor".into(),
                weight: 100.0,
                direction: "higher_is_better".into(),
                min_value: None,
                cap_value: None,
            }],
        };
        assert!(validate_config(&cfg, Path::new("x.toml")).is_err());
    }

    #[test]
    fn rejects_weight_sum_not_100_for_weighted_average() {
        let cfg = ScoreConfigFile {
            scoring: ScoringSection {
                method: "weighted_average".into(),
                pass_threshold: Some(60.0),
                decay: None,
            },
            disqualifiers: BTreeMap::new(),
            disqualifier_rules: vec![],
            metrics: vec![MetricConfig {
                name: "profit_factor".into(),
                weight: 50.0,
                direction: "higher_is_better".into(),
                min_value: None,
                cap_value: None,
            }],
        };
        assert!(validate_config(&cfg, Path::new("x.toml")).is_err());
    }
}
