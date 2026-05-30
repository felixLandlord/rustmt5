use std::fs;
use std::path::{Path, PathBuf};

use super::error::{path_buf_display, MetricsError, Result};
use super::types::{MetricsFile, ReportEntry};
use super::validate;

pub fn default_output_path(report_path: &Path) -> PathBuf {
    let stem = report_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("report");
    PathBuf::from("output/metrics").join(format!("{stem}.json"))
}

pub fn resolve_output_path(report_path: &Path, output: Option<PathBuf>) -> PathBuf {
    output.unwrap_or_else(|| default_output_path(report_path))
}

pub fn write_report(
    report_path: &Path,
    entry: ReportEntry,
    output: Option<PathBuf>,
    append: Option<&Path>,
) -> Result<(PathBuf, u32, Vec<u32>)> {
    let out_path = if let Some(append_path) = append {
        append_path.to_path_buf()
    } else {
        resolve_output_path(report_path, output)
    };

    let mut file = if let Some(append_path) = append {
        if !append_path.exists() {
            return Err(MetricsError::AppendFileNotFound(path_buf_display(
                &append_path.to_path_buf(),
            )));
        }
        load_metrics_file(append_path)?
    } else {
        MetricsFile { reports: vec![] }
    };

    let duplicate_of = ReportEntry::duplicate_ids_in(&file.reports, &entry);

    let next_id = file
        .reports
        .iter()
        .map(|r| r.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut entry = entry;
    entry.id = next_id;
    file.reports.push(entry);

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| MetricsError::InvalidJson(e.to_string()))?;
    }

    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| MetricsError::InvalidJson(e.to_string()))?;
    fs::write(&out_path, json).map_err(|e| MetricsError::InvalidJson(e.to_string()))?;

    Ok((out_path, next_id, duplicate_of))
}

pub fn load_metrics_file(path: &Path) -> Result<MetricsFile> {
    if !path.exists() {
        return Err(MetricsError::FileNotFound(path_buf_display(
            &path.to_path_buf(),
        )));
    }
    let content = fs::read_to_string(path).map_err(|e| MetricsError::InvalidJson(e.to_string()))?;
    let file: MetricsFile =
        serde_json::from_str(&content).map_err(|e| MetricsError::InvalidJson(e.to_string()))?;
    validate::validate_metrics_file(&file, &path_buf_display(&path.to_path_buf()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::schema;
    use crate::metrics::types::{PeriodSettings, ReportEntry, ReportSettings};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn minimal_entry() -> ReportEntry {
        let mut results = BTreeMap::new();
        for key in crate::metrics::schema::REQUIRED_RESULT_KEYS {
            let v = if key.ends_with("_holding_time") {
                json!("00:00:01")
            } else if schema::is_integer_key(key) {
                json!(1)
            } else {
                json!(1.0)
            };
            results.insert((*key).into(), v);
        }
        ReportEntry {
            id: 0,
            settings: ReportSettings {
                expert: "ea".into(),
                symbol: "EURUSD".into(),
                period: PeriodSettings {
                    timeframe: "H1".into(),
                    from_date: "2024.01.01".into(),
                    to_date: "2024.12.31".into(),
                },
                inputs: BTreeMap::new(),
                company: "co".into(),
                currency: "USD".into(),
                initial_deposit: 10000.0,
                leverage: "1:100".into(),
            },
            results,
        }
    }

    #[test]
    fn append_increments_report_id() {
        let dir = std::env::temp_dir().join("rustmt5_metrics_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("metrics.json");
        let report = Path::new("report.htm");

        let (_, id1, d1) = write_report(report, minimal_entry(), Some(path.clone()), None).unwrap();
        let (_, id2, d2) = write_report(report, minimal_entry(), None, Some(&path)).unwrap();
        let (_, id3, d3) = write_report(report, minimal_entry(), None, Some(&path)).unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        assert!(d1.is_empty());
        assert_eq!(d2, vec![1]);
        assert_eq!(d3, vec![2, 1]);
        let file = load_metrics_file(&path).unwrap();
        assert_eq!(file.reports.len(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_ids_sorted_descending() {
        let a = minimal_entry();
        let mut b = minimal_entry();
        b.results.insert("profit_factor".into(), json!(2.0));
        let existing = vec![
            ReportEntry { id: 1, ..a.clone() },
            ReportEntry { id: 5, ..a.clone() },
            ReportEntry { id: 3, ..b },
        ];
        assert_eq!(ReportEntry::duplicate_ids_in(&existing, &a), vec![5, 1]);
    }
}
