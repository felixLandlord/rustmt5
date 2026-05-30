use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsFile {
    #[serde(rename = "report(s)")]
    pub reports: Vec<ReportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEntry {
    pub id: u32,
    pub settings: ReportSettings,
    pub results: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSettings {
    pub expert: String,
    pub symbol: String,
    pub period: PeriodSettings,
    pub inputs: BTreeMap<String, String>,
    pub company: String,
    pub currency: String,
    pub initial_deposit: f64,
    pub leverage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodSettings {
    pub timeframe: String,
    pub from_date: String,
    pub to_date: String,
}
