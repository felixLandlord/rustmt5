use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;
use crate::wine_output::filter_wine_noise;

pub fn run(file: &Path) -> Result<()> {
    validate_ini_file(file)?;

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;
    let ini_content = fs::read_to_string(file)?;
    let report_name = parse_report_name(&ini_content);

    eprintln!("Launching strategy tester with {}...", file.display());

    let output = paths
        .wine_command()
        .arg(&paths.terminal)
        .arg(format!("/config:{wine_path}"))
        .output()
        .map_err(|e| Error::TestFailed {
            detail: format!("failed to launch Wine: {e}"),
        })?;

    let stdout = filter_wine_noise(&String::from_utf8_lossy(&output.stdout));
    let stderr = filter_wine_noise(&String::from_utf8_lossy(&output.stderr));

    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }

    if !output.status.success() {
        return Err(Error::TestFailed {
            detail: format!(
                "terminal64 exited with {}. Common causes: MT5 already running, Expert not found in MQL5/Experts/, or missing historical data for the symbol/timeframe.",
                output.status
            ),
        });
    }

    print_report_location(&paths, report_name.as_deref());
    Ok(())
}

/// Read `Report=` from the `[Tester]` section (key lookup is case-insensitive).
pub(crate) fn parse_report_name(ini_content: &str) -> Option<String> {
    ini_content.lines().map(str::trim).find_map(|line| {
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim().eq_ignore_ascii_case("Report") {
            let name = value.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        } else {
            None
        }
    })
}

/// Strategy tester reports are written next to `terminal64.exe` (MT5 install dir).
pub(crate) fn locate_report(install_dir: &Path, basename: &str) -> Option<PathBuf> {
    for ext in ["htm", "html"] {
        let path = install_dir.join(format!("{basename}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn print_report_location(paths: &Mt5Paths, report_name: Option<&str>) {
    let install_dir = paths.install_dir();

    let Some(name) = report_name else {
        eprintln!(
            "Strategy tester finished. No Report= in .ini — set Report=my_report under [Tester] to name the output."
        );
        eprintln!("Reports are typically saved in: {}", install_dir.display());
        return;
    };

    let Some(report_path) = locate_report(&install_dir, name) else {
        eprintln!("Strategy tester finished, but report file was not found.");
        eprintln!("  Expected: {}/{}.htm", install_dir.display(), name);
        eprintln!("  Check Report= in your .ini and that the test completed successfully.");
        return;
    };

    eprintln!("Strategy tester finished.");
    eprintln!("Report: {}", report_path.display());

    let related: Vec<_> = fs::read_dir(&install_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(name) && n != report_path.file_name().and_then(|f| f.to_str()).unwrap_or(""))
        })
        .collect();

    if !related.is_empty() {
        eprintln!("Related files in the same directory:");
        for path in related {
            eprintln!("  {}", path.display());
        }
    }
}

pub(crate) fn validate_ini_file(file: &Path) -> Result<()> {
    if !file.exists() {
        return Err(Error::FileNotFound { path: file.to_path_buf() });
    }

    match file.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("ini") => {}
        other => {
            return Err(Error::InvalidExtension {
                expected: ".ini",
                got: other.map(String::from),
            });
        }
    }

    let content = fs::read_to_string(file).map_err(|e| Error::InvalidIniFile {
        reason: format!("could not read file: {e}"),
    })?;

    if !content.lines().any(|line| line.trim().eq_ignore_ascii_case("[tester]")) {
        return Err(Error::InvalidIniFile {
            reason: "missing [Tester] section".into(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_ini(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn rejects_missing_file() {
        let result = validate_ini_file(Path::new("/nonexistent/backtest.ini"));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn rejects_wrong_extension() {
        let path = temp_ini("rustmt5_test.txt", "[Tester]\nExpert=MyEA\n");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidExtension { expected: ".ini", .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_missing_tester_section() {
        let path = temp_ini("rustmt5_no_section.ini", "[Common]\nLogin=1000\n");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_valid_ini() {
        let path = temp_ini("rustmt5_valid.ini", "[Tester]\nExpert=MyEA\nSymbol=EURUSD\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_section_case_insensitive() {
        let path = temp_ini("rustmt5_case.ini", "[tester]\nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_section_with_surrounding_content() {
        let content = "[Common]\nLogin=1000\n\n[Tester]\nExpert=MyEA\nSymbol=EURUSD\n\n[Charts]\nMaxBars=5000\n";
        let path = temp_ini("rustmt5_multi_section.ini", content);
        assert!(validate_ini_file(&path).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_tester_as_substring() {
        let path = temp_ini("rustmt5_substr.ini", "MyTester=true\nSomething=1\n");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_file() {
        let path = temp_ini("rustmt5_empty.ini", "");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_no_extension() {
        let path = std::env::temp_dir().join("rustmt5_test_ini_no_ext");
        std::fs::write(&path, "[Tester]\nExpert=MyEA\n").unwrap();
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidExtension { got: None, .. })));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_with_whitespace_around() {
        let path = temp_ini("rustmt5_ws.ini", "  [Tester]  \nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn accepts_ini_case_insensitive_extension() {
        let path = temp_ini("rustmt5_case_ext.INI", "[Tester]\nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_path() {
        let result = validate_ini_file(Path::new(""));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn parse_report_name_reads_tester_field() {
        let ini = "[Tester]\nExpert=MyEA\nReport=strategy_report\n";
        assert_eq!(
            parse_report_name(ini).as_deref(),
            Some("strategy_report")
        );
    }

    #[test]
    fn parse_report_name_is_case_insensitive() {
        let ini = "[Tester]\nreport=MyReport\n";
        assert_eq!(parse_report_name(ini).as_deref(), Some("MyReport"));
    }

    #[test]
    fn parse_report_name_returns_none_when_missing() {
        let ini = "[Tester]\nExpert=MyEA\n";
        assert!(parse_report_name(ini).is_none());
    }

    #[test]
    fn locate_report_finds_htm_in_install_dir() {
        let dir = std::env::temp_dir().join("rustmt5_report_locate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let report = dir.join("my_report.htm");
        std::fs::write(&report, "<html></html>").unwrap();

        let found = locate_report(&dir, "my_report").unwrap();
        assert_eq!(found, report);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
