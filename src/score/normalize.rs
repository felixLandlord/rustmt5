use crate::metrics::ALLOWED_SCORE_METRICS;
use crate::score::config::MetricConfig;

/// Industry-standard bounds (min, max) for normalization.
pub fn default_bounds(metric: &str) -> (f64, f64) {
    match metric {
        "profit_factor" => (0.0, 5.0),
        "sharpe_ratio" => (-5.0, 5.0),
        "recovery_factor" => (-10.0, 10.0),
        "expected_payoff" => (-0.5, 1.0),
        "z_score_%" => (0.0, 100.0),
        "balance_drawdown_maximal_%" | "equity_drawdown_maximal_%" => (0.0, 100.0),
        "long_trades_won_%" | "short_trades_won_%" => (0.0, 100.0),
        "AHPR_%" | "GHPR_%" => (-100.0, 100.0),
        "history_quality_%" => (0.0, 100.0),
        "margin_level_%" => (0.0, 2_000_000.0),
        _ => (0.0, 100.0),
    }
}

/// Normalize a raw metric value to 0–100 scale.
pub fn normalize_metric(cfg: &MetricConfig, raw: f64) -> f64 {
    let mut value = raw;
    if let Some(min) = cfg.min_value {
        if value < min {
            return 0.0;
        }
        value = value.max(min);
    }
    if let Some(cap) = cfg.cap_value {
        value = value.min(cap);
    }

    let (bound_min, bound_max) = default_bounds(&cfg.name);
    if bound_max <= bound_min {
        return 0.0;
    }

    let mut norm = (value - bound_min) / (bound_max - bound_min);
    norm = norm.clamp(0.0, 1.0);

    if cfg.direction == "lower_is_better" {
        norm = 1.0 - norm;
    }

    norm * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_metrics_list_non_empty() {
        assert!(ALLOWED_SCORE_METRICS.contains(&"profit_factor"));
    }
}
