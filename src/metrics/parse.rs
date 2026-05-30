use std::collections::BTreeMap;
use std::path::Path;

use regex::Regex;
use serde_json::{json, Value};

use super::error::{MetricsError, Result};
use super::types::{PeriodSettings, ReportEntry, ReportSettings};

/// Parse an MT5 strategy tester HTML report into a report entry (without id).
pub fn parse_html_report(html: &str, _report_stem: &str) -> Result<ReportEntry> {
    let rows = extract_label_value_rows(html)?;
    let settings = parse_settings(&rows)?;
    let mut results = parse_results(&rows)?;
    parse_deals_totals(html, &mut results)?;

    if results.len() < super::schema::REQUIRED_RESULT_KEYS.len() {
        // fill missing from row scan may have gaps — validation will catch
    }

    Ok(ReportEntry {
        id: 0,
        settings,
        results,
    })
}

fn extract_label_value_rows(html: &str) -> Result<Vec<(String, Vec<String>)>> {
    let orders_pos = html.find("<b>Orders</b>").unwrap_or(html.len());
    let section = &html[..orders_pos];

    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let mut pair_buffer: Vec<(String, String)> = Vec::new();

    for tr in section.split("<tr").skip(1) {
        let tr = tr.split("</tr>").next().unwrap_or(tr);
        if tr.contains("<b>Orders</b>") {
            break;
        }

        let mut current_label = String::new();
        for td in tr.split("<td").skip(1) {
            let td = td.split("</td>").next().unwrap_or(td);
            let cell = td.split_once('>').map(|(_, rest)| rest).unwrap_or(td);
            let plain = strip_tags(cell).trim().to_string();
            let bold = extract_bold(cell);

            if plain.ends_with(':') && plain.len() > 1 {
                current_label = plain.trim_end_matches(':').trim().to_string();
            }

            if let Some(v) = bold {
                if current_label.is_empty() && v.contains('=') {
                    rows.push(("__input__".into(), vec![v]));
                } else if !current_label.is_empty() {
                    pair_buffer.push((current_label.clone(), v));
                    current_label.clear();
                }
            } else if plain.contains('=') && current_label.is_empty() {
                rows.push(("__input__".into(), vec![plain]));
            }
        }
    }

    // Group sequential pairs into multi-value rows when labels repeat pattern
    for (label, value) in pair_buffer {
        rows.push((label, vec![value]));
    }

    if rows.is_empty() {
        return Err(MetricsError::HtmlParsingFailed(
            "no settings/results rows found in HTML".into(),
        ));
    }

    Ok(rows)
}

fn extract_bold(td: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<b>(.*?)</b>").ok()?;
    re.captures(td).and_then(|cap| {
        let v = strip_tags(cap.get(1)?.as_str()).trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    })
}

fn parse_settings(rows: &[(String, Vec<String>)]) -> Result<ReportSettings> {
    let mut expert = String::new();
    let mut symbol = String::new();
    let mut period_raw = String::new();
    let mut company = String::new();
    let mut currency = String::new();
    let mut initial_deposit = 0.0;
    let mut leverage = String::new();
    let mut inputs = BTreeMap::new();

    for (label, values) in rows {
        if label == "__input__" {
            if let Some(pair) = values.first() {
                if let Some((k, v)) = pair.split_once('=') {
                    inputs.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            continue;
        }
        let val = values.first().map(|s| s.as_str()).unwrap_or("");
        match label.as_str() {
            "Expert" => expert = val.to_string(),
            "Symbol" => symbol = val.to_string(),
            "Period" => period_raw = val.to_string(),
            "Company" => company = val.to_string(),
            "Currency" => currency = val.to_string(),
            "Initial Deposit" => initial_deposit = parse_number(val)?,
            "Leverage" => leverage = val.to_string(),
            l if l.contains("Inputs") => {
                if let Some(pair) = values.first() {
                    if let Some((k, v)) = pair.split_once('=') {
                        inputs.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }
            _ if val.contains('=') && label.is_empty() => {
                if let Some((k, v)) = val.split_once('=') {
                    inputs.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            _ => {}
        }
    }

    if expert.is_empty() || symbol.is_empty() || period_raw.is_empty() {
        return Err(MetricsError::HtmlParsingFailed(
            "missing required settings (expert, symbol, or period)".into(),
        ));
    }

    let period = parse_period(&period_raw)?;

    Ok(ReportSettings {
        expert,
        symbol,
        period,
        inputs,
        company,
        currency,
        initial_deposit,
        leverage,
    })
}

fn parse_period(raw: &str) -> Result<PeriodSettings> {
    let re = Regex::new(r"^(\S+)\s*\((\d{4}\.\d{2}\.\d{2})\s*-\s*(\d{4}\.\d{2}\.\d{2})\)")
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;
    if let Some(cap) = re.captures(raw.trim()) {
        return Ok(PeriodSettings {
            timeframe: cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string(),
            from_date: cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
            to_date: cap.get(3).map(|m| m.as_str()).unwrap_or("").to_string(),
        });
    }
    Err(MetricsError::HtmlParsingFailed(format!(
        "could not parse Period: {raw}"
    )))
}

fn parse_results(rows: &[(String, Vec<String>)]) -> Result<BTreeMap<String, Value>> {
    let mut map = BTreeMap::new();

    for (label, values) in rows {
        if label == "__input__" {
            continue;
        }
        if matches!(
            label.as_str(),
            "Expert" | "Symbol" | "Period" | "Company" | "Currency" | "Initial Deposit" | "Leverage"
        ) || label.contains("Inputs")
        {
            continue;
        }

        apply_result_row(label, values, &mut map)?;
    }

    Ok(map)
}

fn apply_result_row(label: &str, values: &[String], map: &mut BTreeMap<String, Value>) -> Result<()> {
    let label = label.trim();
    match label {
        "History Quality" => {
            map.insert("history_quality_%".into(), json!(parse_percent_int(values.first().unwrap_or(&"".into()))?));
        }
        "Bars" => insert_from_row(map, "bars", values, 0)?,
        "Ticks" => insert_from_row(map, "ticks", values, 0)?,
        "Symbols" => insert_from_row(map, "symbols", values, 0)?,
        "Total Net Profit" => insert_from_row(map, "total_net_profit", values, 0)?,
        "Balance Drawdown Absolute" => insert_from_row(map, "balance_drawdown_absolute", values, 0)?,
        "Equity Drawdown Absolute" => insert_from_row(map, "equity_drawdown_absolute", values, 0)?,
        "Gross Profit" => insert_from_row(map, "gross_profit", values, 0)?,
        "Gross Loss" => insert_from_row(map, "gross_loss", values, 0)?,
        "Profit Factor" => insert_from_row(map, "profit_factor", values, 0)?,
        "Expected Payoff" => insert_from_row(map, "expected_payoff", values, 0)?,
        "Margin Level" => insert_from_row(map, "margin_level_%", values, 0)?,
        "Recovery Factor" => insert_from_row(map, "recovery_factor", values, 0)?,
        "Sharpe Ratio" => insert_from_row(map, "sharpe_ratio", values, 0)?,
        "LR Correlation" => insert_from_row(map, "lr_correlation", values, 0)?,
        "LR Standard Error" => insert_from_row(map, "lr_standard_error", values, 0)?,
        "OnTester result" => insert_from_row(map, "ontester_result", values, 0)?,
        "Total Trades" => insert_from_row(map, "total_trades", values, 0)?,
        "Total Deals" => insert_from_row(map, "total_deals", values, 0)?,
        "Balance Drawdown Maximal" => parse_pair_value_percent(map, "balance_drawdown_maximal", "balance_drawdown_maximal_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Equity Drawdown Maximal" => parse_pair_value_percent(map, "equity_drawdown_maximal", "equity_drawdown_maximal_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Balance Drawdown Relative" => parse_relative_drawdown(map, "balance", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Equity Drawdown Relative" => parse_relative_drawdown(map, "equity", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "AHPR" => parse_ahpr_ghpr(map, "AHPR", "AHPR_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "GHPR" => parse_ahpr_ghpr(map, "GHPR", "GHPR_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Z-Score" => parse_z_score(map, values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Short Trades (won %)" => parse_trades_won(map, "short_trades", "short_trades_won_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Long Trades (won %)" => parse_trades_won(map, "long_trades", "long_trades_won_%", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Profit Trades (% of total)" => parse_trades_won(map, "profit_trades", "profit_trades_% (of total)", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Loss Trades (% of total)" => parse_trades_won(map, "loss_trades", "loss_trades_% (of total)", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Largest profit trade" => insert_from_row(map, "largest_profit_trade", values, 0)?,
        "Largest loss trade" => insert_from_row(map, "largest_loss_trade", values, 0)?,
        "Average profit trade" => insert_from_row(map, "average_profit_trade", values, 0)?,
        "Average loss trade" => insert_from_row(map, "average_loss_trade", values, 0)?,
        "Maximum consecutive wins ($)" => parse_consecutive_win_loss(map, true, values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Maximum consecutive losses ($)" => parse_consecutive_win_loss(map, false, values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Maximal consecutive profit (count)" => parse_maximal_consecutive(map, true, values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Maximal consecutive loss (count)" => parse_maximal_consecutive(map, false, values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Average consecutive wins" => insert_from_row(map, "average_consecutive_wins", values, 0)?,
        "Average consecutive losses" => insert_from_row(map, "average_consecutive_losses", values, 0)?,
        "Correlation (Profits,MFE)" => insert_from_row(map, "correlation (Profits, MFE)", values, 0)?,
        "Correlation (Profits,MAE)" => insert_from_row(map, "correlation (Profits, MAE)", values, 0)?,
        "Correlation (MFE,MAE)" => insert_from_row(map, "correlation (MFE, MAE)", values, 0)?,
        "Minimal position holding time" => insert_string(map, "minimal_positon_holding_time", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Maximal position holding time" => insert_string(map, "maximal_positon_holding_time", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        "Average position holding time" => insert_string(map, "average_positon_holding_time", values.get(0).map(|s| s.as_str()).unwrap_or(""))?,
        _ => {
            // Multi-metric rows: Bars | Ticks | Symbols
            if label.contains("Bars") && values.len() >= 3 {
                insert_num(map, "bars", &values[0])?;
                insert_num(map, "ticks", &values[1])?;
                insert_num(map, "symbols", &values[2])?;
            } else if label.contains("Total Net Profit") && values.len() >= 3 {
                insert_num(map, "total_net_profit", &values[0])?;
                insert_num(map, "balance_drawdown_absolute", &values[1])?;
                insert_num(map, "equity_drawdown_absolute", &values[2])?;
            } else if label.contains("Gross Profit") && values.len() >= 3 {
                insert_num(map, "gross_profit", &values[0])?;
                parse_pair_value_percent(map, "balance_drawdown_maximal", "balance_drawdown_maximal_%", &values[1])?;
                parse_pair_value_percent(map, "equity_drawdown_maximal", "equity_drawdown_maximal_%", &values[2])?;
            } else if label.contains("Gross Loss") && values.len() >= 3 {
                insert_num(map, "gross_loss", &values[0])?;
                parse_relative_drawdown(map, "balance", &values[1])?;
                parse_relative_drawdown(map, "equity", &values[2])?;
            } else if label.contains("Profit Factor") && values.len() >= 3 {
                insert_num(map, "profit_factor", &values[0])?;
                insert_num(map, "expected_payoff", &values[1])?;
                insert_num(map, "margin_level_%", &values[2])?;
            } else if label.contains("Recovery Factor") && values.len() >= 3 {
                insert_num(map, "recovery_factor", &values[0])?;
                insert_num(map, "sharpe_ratio", &values[1])?;
                parse_z_score(map, &values[2])?;
            } else if label.contains("Total Trades") && values.len() >= 3 {
                insert_num(map, "total_trades", &values[0])?;
                parse_trades_won(map, "short_trades", "short_trades_won_%", &values[1])?;
                parse_trades_won(map, "long_trades", "long_trades_won_%", &values[2])?;
            } else if label.contains("Total Deals") && values.len() >= 3 {
                insert_num(map, "total_deals", &values[0])?;
                parse_trades_won(map, "profit_trades", "profit_trades_% (of total)", &values[1])?;
                parse_trades_won(map, "loss_trades", "loss_trades_% (of total)", &values[2])?;
            } else if label.contains("Correlation") && values.len() >= 3 {
                insert_num(map, "correlation (Profits, MFE)", &values[0])?;
                insert_num(map, "correlation (Profits, MAE)", &values[1])?;
                insert_num(map, "correlation (MFE, MAE)", &values[2])?;
            } else if label.contains("holding time") && values.len() >= 3 {
                insert_string(map, "minimal_positon_holding_time", &values[0])?;
                insert_string(map, "maximal_positon_holding_time", &values[1])?;
                insert_string(map, "average_positon_holding_time", &values[2])?;
            }
        }
    }
    Ok(())
}

fn parse_deals_totals(html: &str, map: &mut BTreeMap<String, Value>) -> Result<()> {
    let deals_pos = html.find("<b>Deals</b>").ok_or_else(|| {
        MetricsError::HtmlParsingFailed("Deals section not found".into())
    })?;
    let tail = &html[deals_pos..];
    let sum_re = Regex::new(
        r#"(?is)<td\s+nowrap\s+colspan="8"\s+style="height:\s*30px"></td>\s*<td\s+nowrap><b>([^<]*)</b></td>\s*<td\s+nowrap><b>([^<]*)</b></td>\s*<td\s+nowrap><b>([^<]*)</b></td>\s*<td\s+nowrap><b>([^<]*)</b></td>"#,
    )
    .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;

    if let Some(cap) = sum_re.captures(tail) {
        insert_num(map, "commission", cap.get(1).map(|m| m.as_str()).unwrap_or("0"))?;
        insert_num(map, "swap", cap.get(2).map(|m| m.as_str()).unwrap_or("0"))?;
        insert_num(map, "profit", cap.get(3).map(|m| m.as_str()).unwrap_or("0"))?;
        insert_num(map, "balance", cap.get(4).map(|m| m.as_str()).unwrap_or("0"))?;
    }
    Ok(())
}

fn insert_from_row(map: &mut BTreeMap<String, Value>, key: &str, values: &[String], idx: usize) -> Result<()> {
    let v = values.get(idx).map(|s| s.as_str()).unwrap_or("");
    insert_num(map, key, v)
}

fn insert_num(map: &mut BTreeMap<String, Value>, key: &str, raw: &str) -> Result<()> {
    let n = parse_number(raw)?;
    if super::schema::is_integer_key(key) {
        map.insert(key.into(), json!(n.round() as i64));
    } else if super::schema::is_percent_key(key) {
        map.insert(key.into(), json!(n));
    } else {
        map.insert(key.into(), json!(n));
    }
    Ok(())
}

fn insert_string(map: &mut BTreeMap<String, Value>, key: &str, raw: &str) -> Result<()> {
    map.insert(key.into(), Value::String(raw.trim().to_string()));
    Ok(())
}

fn parse_number(raw: &str) -> Result<f64> {
    let cleaned = raw
        .replace('\u{00a0}', " ")
        .replace(' ', "")
        .replace(',', "")
        .trim_end_matches('%')
        .to_string();
    if cleaned.eq_ignore_ascii_case("n/a") || cleaned.is_empty() {
        return Err(MetricsError::HtmlParsingFailed(format!("invalid numeric: {raw}")));
    }
    cleaned
        .parse::<f64>()
        .map_err(|_| MetricsError::HtmlParsingFailed(format!("invalid numeric: {raw}")))
}

fn parse_percent_int(raw: &str) -> Result<i64> {
    Ok(parse_number(raw)?.round() as i64)
}

fn parse_pair_value_percent(
    map: &mut BTreeMap<String, Value>,
    value_key: &str,
    pct_key: &str,
    raw: &str,
) -> Result<()> {
    let re = Regex::new(r"^([^(]+)\(([^)]+)\)").unwrap();
    let raw = raw.trim();
    if let Some(cap) = re.captures(raw) {
        insert_num(map, value_key, cap.get(1).map(|m| m.as_str()).unwrap_or(""))?;
        insert_num(map, pct_key, cap.get(2).map(|m| m.as_str()).unwrap_or(""))?;
    } else {
        insert_num(map, value_key, raw)?;
    }
    Ok(())
}

fn parse_relative_drawdown(map: &mut BTreeMap<String, Value>, prefix: &str, raw: &str) -> Result<()> {
    let re = Regex::new(r"^([^(]+)\(([^)]+)\)").unwrap();
    let pct_key = format!("{prefix}_drawdown_relative_%");
    let val_key = format!("{prefix}_drawdown_relative");
    let raw = raw.trim();
    if let Some(cap) = re.captures(raw) {
        insert_num(map, &pct_key, cap.get(1).map(|m| m.as_str()).unwrap_or(""))?;
        insert_num(map, &val_key, cap.get(2).map(|m| m.as_str()).unwrap_or(""))?;
    } else {
        insert_num(map, &val_key, raw)?;
    }
    Ok(())
}

fn parse_ahpr_ghpr(map: &mut BTreeMap<String, Value>, val_key: &str, pct_key: &str, raw: &str) -> Result<()> {
    let re = Regex::new(r"^([^(]+)\(([^)]+)\)").unwrap();
    if let Some(cap) = re.captures(raw.trim()) {
        insert_num(map, val_key, cap.get(1).map(|m| m.as_str()).unwrap_or(""))?;
        insert_num(map, pct_key, cap.get(2).map(|m| m.as_str()).unwrap_or(""))?;
    } else {
        insert_num(map, val_key, raw)?;
    }
    Ok(())
}

fn parse_z_score(map: &mut BTreeMap<String, Value>, raw: &str) -> Result<()> {
    let re = Regex::new(r"^([^(]+)\(([^)]+)\)").unwrap();
    if let Some(cap) = re.captures(raw.trim()) {
        insert_num(map, "z_score", cap.get(1).map(|m| m.as_str()).unwrap_or(""))?;
        insert_num(map, "z_score_%", cap.get(2).map(|m| m.as_str()).unwrap_or(""))?;
    } else {
        insert_num(map, "z_score", raw)?;
    }
    Ok(())
}

fn parse_trades_won(
    map: &mut BTreeMap<String, Value>,
    count_key: &str,
    pct_key: &str,
    raw: &str,
) -> Result<()> {
    let re = Regex::new(r"^(\d+)\s*\(([^)]+)\)").unwrap();
    if let Some(cap) = re.captures(raw.trim()) {
        map.insert(count_key.into(), json!(cap.get(1).unwrap().as_str().parse::<i64>().unwrap_or(0)));
        insert_num(map, pct_key, cap.get(2).map(|m| m.as_str()).unwrap_or(""))?;
    } else {
        insert_num(map, count_key, raw)?;
    }
    Ok(())
}

fn parse_consecutive_win_loss(map: &mut BTreeMap<String, Value>, wins: bool, raw: &str) -> Result<()> {
    let re = Regex::new(r"^(\d+)\s*\(([^)]+)\)").unwrap();
    if let Some(cap) = re.captures(raw.trim()) {
        let count = cap.get(1).unwrap().as_str();
        let amount = cap.get(2).unwrap().as_str();
        if wins {
            map.insert("maximum_consecutive_wins".into(), json!(count.parse::<i64>().unwrap_or(0)));
            insert_num(map, "amount_from_maximum_consecutive_wins", amount)?;
        } else {
            map.insert("maximum_consecutive_losses".into(), json!(count.parse::<i64>().unwrap_or(0)));
            insert_num(map, "amount_from_maximum_consecutive_losses", amount)?;
        }
    }
    Ok(())
}

fn parse_maximal_consecutive(map: &mut BTreeMap<String, Value>, profit: bool, raw: &str) -> Result<()> {
    let re = Regex::new(r"^([^(]+)\((\d+)\)").unwrap();
    if let Some(cap) = re.captures(raw.trim()) {
        let val = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let count = cap.get(2).map(|m| m.as_str()).unwrap_or("0");
        if profit {
            insert_num(map, "maximal_consecutive_profit", val)?;
            map.insert("maximal_consecutive_profit_count".into(), json!(count.parse::<i64>().unwrap_or(0)));
        } else {
            insert_num(map, "maximal_consecutive_loss", val)?;
            map.insert("maximal_consecutive_loss_count".into(), json!(count.parse::<i64>().unwrap_or(0)));
        }
    }
    Ok(())
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").to_string()
}

pub fn report_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample_report_html() -> Option<String> {
        let path = Path::new("examples/output/test/strategy_report.htm");
        if !path.exists() {
            return None;
        }
        crate::text_decode::read_text_file(path).ok()
    }

    #[test]
    fn parses_real_strategy_report_when_present() {
        let Some(html) = sample_report_html() else {
            return;
        };
        let entry = parse_html_report(&html, "strategy_report").expect("parse");
        assert_eq!(entry.settings.expert, "strategy");
        assert_eq!(entry.settings.symbol, "USA500");
        assert!(entry.results.contains_key("profit_factor"));
        assert!(entry.results.contains_key("balance"));
    }

    #[test]
    fn parse_period_extracts_dates() {
        let period = parse_period("M5 (2026.04.01 - 2026.05.01)").unwrap();
        assert_eq!(period.timeframe, "M5");
        assert_eq!(period.from_date, "2026.04.01");
        assert_eq!(period.to_date, "2026.05.01");
    }
}
