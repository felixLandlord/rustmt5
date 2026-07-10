//! Extract paired trades from the MT5 HTML report "Deals" section,
//! enriched with SL/TP distances from the "Orders" section.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::{MetricsError, Result};

const DEAL_COLUMNS: usize = 13;
const ORDER_COLUMNS: usize = 11;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeRecord {
    pub count: u32,
    pub in_time: String,
    pub out_time: String,
    #[serde(rename = "type")]
    pub trade_type: String,
    pub volume: Value,
    pub in_price: Value,
    pub out_price: Value,
    pub commission: Value,
    pub swap: Value,
    pub profit: Value,
    pub balance: Value,
    /// Absolute distance from entry price to stop-loss (from Orders S/L).
    pub sl: Value,
    /// Absolute distance from entry price to take-profit (from Orders T/P).
    pub tp: Value,
}

#[derive(Debug, Clone)]
struct DealRow {
    time: String,
    deal: String,
    direction: String,
    trade_type: String,
    volume: String,
    price: String,
    order: String,
    commission: String,
    swap: String,
    profit: String,
    balance: String,
}

#[derive(Debug, Clone)]
struct OrderLevels {
    sl: Option<f64>,
    tp: Option<f64>,
}

/// Parse Deals + Orders and pair in/out rows into trade records with SL/TP distances.
pub fn extract_trades(html: &str) -> Result<Vec<TradeRecord>> {
    let orders = parse_order_levels(html)?;
    let deals = parse_deal_rows(html)?;
    build_trades(&deals, &orders)
}

fn parse_order_levels(html: &str) -> Result<HashMap<String, OrderLevels>> {
    let orders_pos = html.find("<b>Orders</b>").ok_or_else(|| {
        MetricsError::HtmlParsingFailed("Could not find an 'Orders' section in the report".into())
    })?;
    let deals_pos = html[orders_pos..]
        .find("<b>Deals</b>")
        .map(|i| orders_pos + i)
        .unwrap_or(html.len());
    let section = &html[orders_pos..deals_pos];

    let tr_re = Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>")
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;
    let td_re = Regex::new(r"(?is)<td[^>]*>(.*?)</td>")
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;

    let mut map = HashMap::new();
    for tr in tr_re.captures_iter(section) {
        let row_html = tr.get(1).map(|m| m.as_str()).unwrap_or("");
        let cells: Vec<String> = td_re
            .captures_iter(row_html)
            .map(|c| strip_tags(c.get(1).map(|m| m.as_str()).unwrap_or("")))
            .collect();

        if cells.len() != ORDER_COLUMNS {
            continue;
        }
        // Header: Order column is "Order"
        if cells[1] == "Order" {
            continue;
        }

        let order_id = cells[1].clone();
        let sl = parse_optional_f64(&cells[6]);
        let tp = parse_optional_f64(&cells[7]);
        // Entry orders carry S/L and T/P; exit orders usually leave them blank.
        // Keep the first non-empty levels for each order id.
        let entry = map.entry(order_id).or_insert(OrderLevels { sl: None, tp: None });
        if entry.sl.is_none() && sl.is_some() {
            entry.sl = sl;
        }
        if entry.tp.is_none() && tp.is_some() {
            entry.tp = tp;
        }
    }

    Ok(map)
}

fn parse_deal_rows(html: &str) -> Result<Vec<DealRow>> {
    let marker = html.rfind("<b>Deals</b>").ok_or_else(|| {
        MetricsError::HtmlParsingFailed("Could not find a 'Deals' section in the report".into())
    })?;
    let section = &html[marker..];

    let tr_re = Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>")
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;
    let td_re = Regex::new(r"(?is)<td[^>]*>(.*?)</td>")
        .map_err(|e| MetricsError::HtmlParsingFailed(e.to_string()))?;

    let mut deals = Vec::new();
    for tr in tr_re.captures_iter(section) {
        let row_html = tr.get(1).map(|m| m.as_str()).unwrap_or("");
        let cells: Vec<String> = td_re
            .captures_iter(row_html)
            .map(|c| strip_tags(c.get(1).map(|m| m.as_str()).unwrap_or("")))
            .collect();

        if cells.len() != DEAL_COLUMNS {
            continue;
        }

        let deal = cells[1].as_str();
        let trade_type = cells[3].as_str();
        if deal == "Deal" {
            continue;
        }
        if trade_type == "balance" {
            continue;
        }

        deals.push(DealRow {
            time: cells[0].clone(),
            deal: cells[1].clone(),
            trade_type: cells[3].clone(),
            direction: cells[4].clone(),
            volume: cells[5].clone(),
            price: cells[6].clone(),
            order: cells[7].clone(),
            commission: cells[8].clone(),
            swap: cells[9].clone(),
            profit: cells[10].clone(),
            balance: cells[11].clone(),
        });
    }

    Ok(deals)
}

fn build_trades(
    deals: &[DealRow],
    orders: &HashMap<String, OrderLevels>,
) -> Result<Vec<TradeRecord>> {
    if deals.len() % 2 != 0 {
        return Err(MetricsError::HtmlParsingFailed(format!(
            "Expected an even number of deal rows to pair, got {}",
            deals.len()
        )));
    }

    let mut trades = Vec::with_capacity(deals.len() / 2);
    for chunk in deals.chunks_exact(2) {
        let entry = &chunk[0];
        let exit = &chunk[1];

        if entry.direction != "in" || exit.direction != "out" {
            return Err(MetricsError::HtmlParsingFailed(format!(
                "Unexpected direction pairing at deals {}/{}: {:?}, {:?}",
                entry.deal, exit.deal, entry.direction, exit.direction
            )));
        }

        let commission = num_or_zero(&entry.commission) + num_or_zero(&exit.commission);
        let swap = num_or_zero(&entry.swap) + num_or_zero(&exit.swap);
        let profit = num_or_zero(&entry.profit) + num_or_zero(&exit.profit);
        let in_price = num_or_zero(&entry.price);

        let levels = orders.get(&entry.order);
        let sl = levels
            .and_then(|l| l.sl)
            .map(|sl_price| distance(in_price, sl_price))
            .unwrap_or(Value::Null);
        let tp = levels
            .and_then(|l| l.tp)
            .map(|tp_price| distance(in_price, tp_price))
            .unwrap_or(Value::Null);

        trades.push(TradeRecord {
            count: (trades.len() as u32) + 1,
            in_time: entry.time.clone(),
            out_time: exit.time.clone(),
            trade_type: entry.trade_type.clone(),
            volume: to_number(&entry.volume),
            in_price: to_number(&entry.price),
            out_price: to_number(&exit.price),
            commission: json_number(commission),
            swap: json_number(swap),
            profit: json_number(profit),
            balance: to_number(&exit.balance),
            sl,
            tp,
        });
    }

    Ok(trades)
}

fn strip_tags(cell_html: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(cell_html, "").trim().to_string()
}

fn parse_optional_f64(value: &str) -> Option<f64> {
    let value = value.trim().replace(',', "");
    if value.is_empty() {
        return None;
    }
    value.parse::<f64>().ok()
}

fn to_number(value: &str) -> Value {
    let value = value.trim();
    if value.is_empty() {
        return Value::Null;
    }
    let cleaned = value.replace(',', "");
    if cleaned.contains('.') {
        cleaned
            .parse::<f64>()
            .map(json_number)
            .unwrap_or_else(|_| Value::String(value.to_string()))
    } else {
        cleaned
            .parse::<i64>()
            .map(|n| Value::Number(n.into()))
            .unwrap_or_else(|_| Value::String(value.to_string()))
    }
}

fn num_or_zero(value: &str) -> f64 {
    match to_number(value) {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn json_number(f: f64) -> Value {
    // Prefer integers when the value is whole (matches Python prototype).
    if f.fract() == 0.0 && f.abs() <= i64::MAX as f64 {
        Value::Number((f as i64).into())
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// Round to 2 decimal places to avoid float noise (e.g. 17.900000000001).
fn round2(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}

fn distance(in_price: f64, level: f64) -> Value {
    json_number(round2((in_price - level).abs()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_html() -> String {
        r#"
<html><body>
<div><b>Orders</b></div>
<table>
<tr><td>Open Time</td><td>Order</td><td>Symbol</td><td>Type</td><td>Volume</td>
<td>Price</td><td>S / L</td><td>T / P</td><td>Time</td><td>State</td><td>Comment</td></tr>
<tr><td>2026.05.01 01:40:00</td><td>2</td><td>USA100</td><td>sell</td><td>0.16 / 0.16</td>
<td>0.00</td><td>27520.70</td><td>27474.75</td><td>2026.05.01 01:40:00</td><td>filled</td><td></td></tr>
<tr><td>2026.05.01 01:51:40</td><td>3</td><td>USA100</td><td>buy</td><td>0.16 / 0.16</td>
<td>0.00</td><td></td><td></td><td>2026.05.01 01:51:40</td><td>filled</td><td>tp</td></tr>
<tr><td>2026.05.01 02:30:00</td><td>4</td><td>USA100</td><td>buy</td><td>0.24 / 0.24</td>
<td>0.00</td><td>27480.00</td><td>27520.00</td><td>2026.05.01 02:30:00</td><td>filled</td><td></td></tr>
<tr><td>2026.05.01 02:45:00</td><td>5</td><td>USA100</td><td>sell</td><td>0.24 / 0.24</td>
<td>0.00</td><td></td><td></td><td>2026.05.01 02:45:00</td><td>filled</td><td>sl</td></tr>
</table>
<div><b>Deals</b></div>
<table>
<tr align="center"><td><b>Time</b></td><td><b>Deal</b></td><td><b>Symbol</b></td>
<td><b>Type</b></td><td><b>Direction</b></td><td><b>Volume</b></td><td><b>Price</b></td>
<td><b>Order</b></td><td><b>Commission</b></td><td><b>Swap</b></td><td><b>Profit</b></td>
<td><b>Balance</b></td><td><b>Comment</b></td></tr>
<tr><td>2026.05.01 00:00:00</td><td>1</td><td></td><td>balance</td><td></td><td></td><td></td>
<td></td><td>0.00</td><td>0.00</td><td>55.00</td><td>55.00</td><td></td></tr>
<tr><td>2026.05.01 01:40:00</td><td>2</td><td>USA100</td><td>sell</td><td>in</td><td>0.16</td><td>27502.80</td>
<td>2</td><td>0.00</td><td>0.00</td><td>0.00</td><td>55.00</td><td></td></tr>
<tr><td>2026.05.01 01:51:40</td><td>3</td><td>USA100</td><td>buy</td><td>out</td><td>0.16</td><td>27482.19</td>
<td>3</td><td>0.00</td><td>0.00</td><td>3.30</td><td>58.30</td><td>tp</td></tr>
<tr><td>2026.05.01 02:30:00</td><td>4</td><td>USA100</td><td>buy</td><td>in</td><td>0.24</td><td>27497.05</td>
<td>4</td><td>0.00</td><td>0.00</td><td>0.00</td><td>58.30</td><td></td></tr>
<tr><td>2026.05.01 02:45:00</td><td>5</td><td>USA100</td><td>sell</td><td>out</td><td>0.24</td><td>27497.05</td>
<td>5</td><td>0.00</td><td>0.00</td><td>0.00</td><td>58.30</td><td>sl</td></tr>
</table>
</body></html>
"#
        .to_string()
    }

    #[test]
    fn extracts_paired_trades_with_sl_tp_distance() {
        let trades = extract_trades(&sample_html()).unwrap();
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].count, 1);
        assert_eq!(trades[0].trade_type, "sell");
        assert_eq!(trades[0].in_price, serde_json::json!(27502.8));
        // |27502.80 - 27520.70| = 17.9, |27502.80 - 27474.75| = 28.05
        assert_eq!(trades[0].sl, serde_json::json!(17.9));
        assert_eq!(trades[0].tp, serde_json::json!(28.05));
        assert_eq!(trades[1].sl, serde_json::json!(17.05));
        assert_eq!(trades[1].tp, serde_json::json!(22.95));
    }

    #[test]
    fn rejects_odd_deal_count() {
        let html = sample_html().replace(
            r#"<tr><td>2026.05.01 02:45:00</td><td>5</td><td>USA100</td><td>sell</td><td>out</td><td>0.24</td><td>27497.05</td>
<td>5</td><td>0.00</td><td>0.00</td><td>0.00</td><td>58.30</td><td>sl</td></tr>"#,
            "",
        );
        let err = extract_trades(&html).unwrap_err();
        assert!(err.to_string().contains("even number"));
    }

    #[test]
    fn rejects_missing_deals_section() {
        let err = extract_trades("<html><b>Orders</b></html>").unwrap_err();
        assert!(err.to_string().contains("Deals"));
    }

    #[test]
    fn extracts_from_real_strategy_report_when_present() {
        let path = std::path::Path::new("z_context/test/strategy_report.htm");
        if !path.exists() {
            return;
        }
        let html = crate::text_decode::read_text_file(path).unwrap();
        let trades = extract_trades(&html).unwrap();
        assert!(!trades.is_empty());
        assert_eq!(trades[0].count, 1);
        assert_eq!(trades[0].trade_type, "buy");
        // in 27510.65, SL 27486.62 → 24.03; TP 27546.69 → 36.04
        assert_eq!(trades[0].sl, serde_json::json!(24.03));
        assert_eq!(trades[0].tp, serde_json::json!(36.04));
        assert!(trades.iter().all(|t| !t.sl.is_null() && !t.tp.is_null()));
    }
}
