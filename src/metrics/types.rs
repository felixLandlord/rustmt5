use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsFile {
    #[serde(rename = "report(s)")]
    pub reports: Vec<ReportEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportEntry {
    pub id: u32,
    pub settings: ReportSettings,
    pub results: BTreeMap<String, Value>,
}

impl ReportEntry {
    /// Compare settings and results, ignoring report id.
    pub fn content_eq(&self, other: &ReportEntry) -> bool {
        self.settings == other.settings && self.results == other.results
    }

    /// IDs of existing reports with identical content (highest id first).
    pub fn duplicate_ids_in(existing: &[ReportEntry], new: &ReportEntry) -> Vec<u32> {
        let mut ids: Vec<u32> = existing
            .iter()
            .filter(|r| r.content_eq(new))
            .map(|r| r.id)
            .collect();
        ids.sort_by(|a, b| b.cmp(a));
        ids
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeriodSettings {
    pub timeframe: String,
    pub from_date: String,
    pub to_date: String,
}
