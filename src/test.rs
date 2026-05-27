use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;
use crate::wine_output::filter_wine_noise;

/// Sentinel used by clap when `--output` is passed without a directory.
pub const OUTPUT_FLAG_INI_DIR: &str = "__INI_DIR__";

/// Local report directory: `output/test/` next to the `.ini`.
pub fn default_report_dest(ini_file: &Path) -> PathBuf {
    ini_file
        .parent()
        .unwrap_or(Path::new("."))
        .join("output/test")
}

/// Resolve `--input` into a copy destination.
///
/// - `None` / `Some("__INI_DIR__")` → `output/test/` next to the `.ini`
/// - `Some("/some/path")`           → explicit directory
pub fn resolve_report_dest(flag: Option<String>, ini_file: &Path) -> Option<PathBuf> {
    let raw = flag.unwrap_or_else(|| OUTPUT_FLAG_INI_DIR.to_string());
    if raw == OUTPUT_FLAG_INI_DIR {
        Some(default_report_dest(ini_file))
    } else {
        Some(PathBuf::from(raw))
    }
}

pub fn run(file: &Path, report_dest: Option<&Path>) -> Result<()> {
    validate_ini_file(file)?;

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;
    let ini_content = fs::read_to_string(file)?;
    let report_name = parse_report_name(&ini_content);

    // Ensure the report subdir exists inside the MT5 install dir so MT5 can write there.
    // Normalise separators: Wine may use `\`; macOS Path needs `/`.
    if let Some(ref name) = report_name {
        let normalised = name.replace('\\', "/");
        if let Some(subdir) = normalised.rfind('/').map(|i| &normalised[..i]).filter(|s| !s.is_empty()) {
            let full_subdir = paths.install_dir().join(subdir);
            if !full_subdir.exists() {
                eprintln!("Creating report directory: {}", full_subdir.display());
                fs::create_dir_all(&full_subdir).map_err(|e| Error::TestFailed {
                    detail: format!(
                        "could not create report directory {}: {e}",
                        full_subdir.display()
                    ),
                })?;
            }
        }
    }

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

    handle_reports(&paths, report_name.as_deref(), report_dest);
    Ok(())
}

/// Locate and copy all report files, then print their final location.
fn handle_reports(paths: &Mt5Paths, report_name: Option<&str>, dest: Option<&Path>) {
    let install_dir = paths.install_dir();

    let Some(name) = report_name else {
        eprintln!("Strategy tester finished.");
        eprintln!(
            "No Report= key in .ini — set Report=<name> under [Tester] to name the output."
        );
        eprintln!("Reports are saved in: {}", install_dir.display());
        return;
    };

    // Report= may use forward or back slashes as separators (Wine normalises to `\`).
    // Replace `\` with `/` before parsing so the macOS Path sees proper components.
    let name_normalized = name.replace('\\', "/");

    // Split into optional subdirectory and stem.
    let (report_subdir, stem): (Option<&str>, &str) = match name_normalized.rfind('/') {
        Some(idx) => (Some(&name_normalized[..idx]), &name_normalized[idx + 1..]),
        None => (None, &name_normalized),
    };

    let report_dir = match report_subdir.filter(|s| !s.is_empty()) {
        Some(sub) => install_dir.join(sub),
        None => install_dir.clone(),
    };

    let html = locate_report(&report_dir, stem);
    let related = find_related_files(&report_dir, stem, html.as_deref());

    if html.is_none() && related.is_empty() {
        eprintln!("Strategy tester finished, but no report files were found.");
        eprintln!("  Expected: {}", report_dir.join(format!("{stem}.htm")).display());
        eprintln!("  Check Report= in your .ini and that the test completed successfully.");
        return;
    }

    eprintln!("Strategy tester finished.");

    if let Some(dest) = dest {
        copy_reports(html.as_deref(), &related, dest, stem);
    } else {
        // No dest supplied at all — just print locations in the MT5 dir.
        if let Some(ref p) = html {
            eprintln!("Report: {}", p.display());
        }
        for p in &related {
            eprintln!("  {}", p.display());
        }
    }
}

fn find_related_files(report_dir: &Path, stem: &str, exclude: Option<&Path>) -> Vec<PathBuf> {
    fs::read_dir(report_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let matches_stem = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(stem));
            let not_excluded = exclude.map_or(true, |ex| p != ex);
            matches_stem && not_excluded && p.is_file()
        })
        .collect()
}

fn copy_reports(html: Option<&Path>, related: &[PathBuf], dest: &Path, stem: &str) {
    if !dest.exists() {
        eprintln!("Creating report destination: {}", dest.display());
        if let Err(e) = fs::create_dir_all(dest) {
            eprintln!("warning: could not create destination directory: {e}");
            if let Some(h) = html {
                eprintln!("  Report remains at: {}", h.display());
            }
            return;
        }
    }

    if !dest.is_dir() {
        eprintln!(
            "warning: report destination is not a directory, skipping copy: {}",
            dest.display()
        );
        return;
    }

    let mut copied = 0usize;
    let mut failed = 0usize;

    let all: Vec<&Path> = html.into_iter().chain(related.iter().map(|p| p.as_path())).collect();
    for src in all {
        let file_name = match src.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dst = dest.join(file_name);
        match fs::copy(src, &dst) {
            Ok(_) => {
                eprintln!("  copied: {}", dst.display());
                copied += 1;
            }
            Err(e) => {
                eprintln!("  warning: could not copy {}: {e}", src.display());
                failed += 1;
            }
        }
    }

    eprintln!(
        "Report: {} file(s) copied to {}{}",
        copied,
        dest.display(),
        if failed > 0 {
            format!(" ({failed} failed)")
        } else {
            String::new()
        }
    );

    // Print the main HTML path in the destination for easy opening.
    let htm_dest = dest.join(format!("{stem}.htm"));
    if htm_dest.exists() {
        eprintln!("Open: {}", htm_dest.display());
    }
    let html_dest = dest.join(format!("{stem}.html"));
    if html_dest.exists() {
        eprintln!("Open: {}", html_dest.display());
    }
}

/// Read `Report=` from the `.ini` (key lookup is case-insensitive).
pub(crate) fn parse_report_name(ini_content: &str) -> Option<String> {
    ini_content.lines().map(str::trim).find_map(|line| {
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            return None;
        }
        let (key, value) = line.split_once('=')?;
        if key.trim().eq_ignore_ascii_case("Report") {
            let name = value.trim();
            if name.is_empty() { None } else { Some(name.to_string()) }
        } else {
            None
        }
    })
}

/// Main HTML report in `report_dir` (`<stem>.htm` or `.html`).
fn locate_report(report_dir: &Path, stem: &str) -> Option<PathBuf> {
    for ext in ["htm", "html"] {
        let p = report_dir.join(format!("{stem}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
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

    fn temp_ini(name: &str, content: &str) -> PathBuf {
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
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_missing_tester_section() {
        let path = temp_ini("rustmt5_no_section.ini", "[Common]\nLogin=1000\n");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_valid_ini() {
        let path = temp_ini("rustmt5_valid.ini", "[Tester]\nExpert=MyEA\nSymbol=EURUSD\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_section_case_insensitive() {
        let path = temp_ini("rustmt5_case.ini", "[tester]\nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_section_with_surrounding_content() {
        let content = "[Common]\nLogin=1000\n\n[Tester]\nExpert=MyEA\nSymbol=EURUSD\n\n[Charts]\nMaxBars=5000\n";
        let path = temp_ini("rustmt5_multi_section.ini", content);
        assert!(validate_ini_file(&path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_tester_as_substring() {
        let path = temp_ini("rustmt5_substr.ini", "MyTester=true\nSomething=1\n");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_file() {
        let path = temp_ini("rustmt5_empty.ini", "");
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidIniFile { .. })));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_no_extension() {
        let path = std::env::temp_dir().join("rustmt5_test_ini_no_ext");
        fs::write(&path, "[Tester]\nExpert=MyEA\n").unwrap();
        let result = validate_ini_file(&path);
        assert!(matches!(result, Err(Error::InvalidExtension { got: None, .. })));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_tester_with_whitespace_around() {
        let path = temp_ini("rustmt5_ws.ini", "  [Tester]  \nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn accepts_ini_case_insensitive_extension() {
        let path = temp_ini("rustmt5_case_ext.INI", "[Tester]\nExpert=MyEA\n");
        assert!(validate_ini_file(&path).is_ok());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn rejects_empty_path() {
        let result = validate_ini_file(Path::new(""));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn parse_report_name_reads_tester_field() {
        let ini = "[Tester]\nExpert=MyEA\nReport=strategy_report\n";
        assert_eq!(parse_report_name(ini).as_deref(), Some("strategy_report"));
    }

    #[test]
    fn parse_report_name_reads_subdirectory_path() {
        let ini = "[Tester]\nReport=rustmt5_report/strategy_report\n";
        assert_eq!(
            parse_report_name(ini).as_deref(),
            Some("rustmt5_report/strategy_report")
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
    fn locate_report_finds_htm_in_dir() {
        let dir = std::env::temp_dir().join("rustmt5_report_locate");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let report = dir.join("my_report.htm");
        fs::write(&report, "<html></html>").unwrap();
        let found = locate_report(&dir, "my_report").unwrap();
        assert_eq!(found, report);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolve_report_dest_defaults_to_output_test() {
        let ini = std::env::temp_dir().join("sub/backtest.ini");
        let dest = resolve_report_dest(None, &ini).unwrap();
        assert_eq!(dest, std::env::temp_dir().join("sub/output/test"));
    }

    #[test]
    fn resolve_report_dest_sentinel_equals_output_test() {
        let ini = std::env::temp_dir().join("sub/backtest.ini");
        let dest = resolve_report_dest(Some(OUTPUT_FLAG_INI_DIR.to_string()), &ini).unwrap();
        assert_eq!(dest, std::env::temp_dir().join("sub/output/test"));
    }

    #[test]
    fn resolve_report_dest_custom_path() {
        let ini = std::env::temp_dir().join("backtest.ini");
        let dest = resolve_report_dest(Some("/tmp/my-reports".into()), &ini).unwrap();
        assert_eq!(dest, PathBuf::from("/tmp/my-reports"));
    }

    #[test]
    fn copy_reports_creates_missing_dest() {
        let src_dir = std::env::temp_dir().join("rustmt5_copy_src_mkdir");
        let dest_dir = std::env::temp_dir().join("rustmt5_copy_dest_new/nested");
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(std::env::temp_dir().join("rustmt5_copy_dest_new"));
        fs::create_dir_all(&src_dir).unwrap();
        let html = src_dir.join("report.htm");
        fs::write(&html, "<html></html>").unwrap();
        copy_reports(Some(&html), &[], &dest_dir, "report");
        assert!(dest_dir.join("report.htm").exists());
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(std::env::temp_dir().join("rustmt5_copy_dest_new"));
    }

    #[test]
    fn copy_reports_copies_files_to_dest() {
        let src_dir = std::env::temp_dir().join("rustmt5_copy_src2");
        let dst_dir = std::env::temp_dir().join("rustmt5_copy_dst2");
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&dst_dir).unwrap();

        let html = src_dir.join("report.htm");
        let png = src_dir.join("report.png");
        fs::write(&html, "<html></html>").unwrap();
        fs::write(&png, "fake-png").unwrap();

        copy_reports(Some(&html), &[png], &dst_dir, "report");

        assert!(dst_dir.join("report.htm").exists());
        assert!(dst_dir.join("report.png").exists());

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dst_dir);
    }
}
