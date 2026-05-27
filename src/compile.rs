use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;

pub fn run(file: &Path, output_dir: Option<&Path>) -> Result<()> {
    validate_mq5_file(file)?;

    if let Some(dir) = output_dir {
        validate_output_dir(dir)?;
    }

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;

    eprintln!("Compiling {}...", file.display());

    let output = paths
        .wine_command()
        .arg(&paths.editor)
        .arg(format!("/compile:{wine_path}"))
        .arg("/log")
        .output()
        .map_err(|e| Error::CompileFailed {
            detail: format!("failed to launch Wine: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let log_path = log_path_for(file);
    let log_content = read_log(&log_path);

    print_compile_output(log_content.as_deref(), &stdout, &stderr);

    evaluate_compile_outcome(
        file,
        output_dir,
        log_content.as_deref(),
        &stdout,
        &stderr,
        output.status.success(),
    )
}

fn log_path_for(mq5: &Path) -> PathBuf {
    mq5.with_extension("log")
}

fn read_log(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn print_compile_output(log: Option<&str>, stdout: &str, stderr: &str) {
    if let Some(log) = log {
        if !log.trim().is_empty() {
            println!("{}", log.trim_end());
            return;
        }
    }

    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
}

fn evaluate_compile_outcome(
    file: &Path,
    output_dir: Option<&Path>,
    log: Option<&str>,
    stdout: &str,
    stderr: &str,
    exit_ok: bool,
) -> Result<()> {
    let combined_terminal = format!("{stdout}{stderr}");

    if let Some(log) = log {
        if let Some((errors, warnings)) = parse_compile_log(log) {
            if errors == 0 {
                if !exit_ok {
                    eprintln!(
                        "Note: MetaEditor exited with a non-zero status, but the compile log reports success."
                    );
                }
                return report_success(file, output_dir, warnings);
            }
            return Err(Error::CompileFailed {
                detail: format!(
                    "compilation finished with {errors} error(s), {warnings} warning(s) (see {})",
                    log_path_for(file).display()
                ),
            });
        }
    }

    if contains_compile_errors(&combined_terminal) {
        return Err(Error::CompileFailed {
            detail: "compilation finished with errors (see output above)".into(),
        });
    }

    if let Some(log) = log {
        if contains_compile_errors(log) {
            return Err(Error::CompileFailed {
                detail: format!(
                    "compilation finished with errors (see {})",
                    log_path_for(file).display()
                ),
            });
        }
    }

    let ex5 = file.with_extension("ex5");
    if ex5.exists() {
        if !exit_ok {
            eprintln!(
                "Note: MetaEditor exited with a non-zero status, but {} was produced.",
                ex5.display()
            );
        }
        return report_success(file, output_dir, 0);
    }

    if !exit_ok {
        return Err(Error::CompileFailed {
            detail: format!(
                "metaeditor64 exited with a non-zero status and no .ex5 was produced (check {})",
                log_path_for(file).display()
            ),
        });
    }

    eprintln!("Done. Check output for results.");
    Ok(())
}

fn report_success(file: &Path, output_dir: Option<&Path>, warnings: u32) -> Result<()> {
    let ex5 = file.with_extension("ex5");

    if let Some(dir) = output_dir {
        if ex5.exists() {
            let dest = dir.join(ex5.file_name().expect("ex5 has a filename"));
            std::fs::copy(&ex5, &dest)?;
            eprintln!("Success: copied to {}", dest.display());
        } else {
            eprintln!("Compile log reports success, but no .ex5 found to copy.");
        }
    } else if ex5.exists() {
        eprintln!("Success: {}", ex5.display());
    } else {
        eprintln!("Compile log reports success (0 errors).");
    }

    if warnings > 0 {
        eprintln!("Completed with {warnings} warning(s).");
    }

    Ok(())
}

/// Parse error and warning counts from a MetaEditor compile log or stdout.
///
/// Recognizes lines like:
/// - `Result: 0 errors, 0 warnings, 623 ms elapsed`
/// - `strategy.mq5 : 0 error(s), 0 warning(s)`
pub(crate) fn parse_compile_log(log: &str) -> Option<(u32, u32)> {
    let mut last = None;
    for line in log.lines() {
        if let Some(counts) = parse_error_counts_line(line.trim()) {
            last = Some(counts);
        }
    }
    last
}

fn parse_error_counts_line(line: &str) -> Option<(u32, u32)> {
    if let Some(rest) = line.strip_prefix("Result:") {
        return parse_errors_and_warnings(rest);
    }

    if line.contains("error(s)") {
        let errors = parse_count_before_keyword(line, "error(s)")?;
        let warnings = parse_count_before_keyword(line, "warning(s)")?;
        return Some((errors, warnings));
    }

    None
}

fn parse_errors_and_warnings(fragment: &str) -> Option<(u32, u32)> {
    let errors = parse_count_before_keyword(fragment, "error")?;
    let warnings = parse_count_before_keyword(fragment, "warning")?;
    Some((errors, warnings))
}

fn parse_count_before_keyword(text: &str, keyword: &str) -> Option<u32> {
    let idx = text.find(keyword)?;
    let before = text[..idx].trim();
    let num = before
        .rsplit(|c: char| c.is_whitespace() || c == ',')
        .find(|s| !s.is_empty())?;
    num.parse().ok()
}

pub(crate) fn validate_mq5_file(file: &Path) -> Result<()> {
    if !file.exists() {
        return Err(Error::FileNotFound { path: file.to_path_buf() });
    }

    match file.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("mq5") => Ok(()),
        other => Err(Error::InvalidExtension {
            expected: ".mq5",
            got: other.map(String::from),
        }),
    }
}

fn validate_output_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Err(Error::FileNotFound { path: dir.to_path_buf() });
    }
    if !dir.is_dir() {
        return Err(Error::InvalidOutputDir { path: dir.to_path_buf() });
    }
    Ok(())
}

pub(crate) fn contains_compile_errors(output: &str) -> bool {
    output.lines().any(|line| {
        let line = line.trim();
        if let Some(pos) = line.find("error(s)") {
            let before = &line[..pos].trim_end();
            if let Some(count_str) = before.rsplit_once(' ').or(before.rsplit_once(':')) {
                if let Ok(n) = count_str.1.trim().parse::<u32>() {
                    return n > 0;
                }
            }
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_LOG: &str = r"
Z:\Users\landlord\Files\me\rustmt5\examples\strategy.mq5 : information: compiling
 : information: code generated
Result: 0 errors, 0 warnings, 623 ms elapsed, cpu='X64 Regular'
";

    #[test]
    fn parse_compile_log_reads_result_line() {
        assert_eq!(parse_compile_log(SAMPLE_LOG), Some((0, 0)));
    }

    #[test]
    fn parse_compile_log_reads_error_summary_line() {
        let log = "strategy.mq5 : 2 error(s), 3 warning(s)\n";
        assert_eq!(parse_compile_log(log), Some((2, 3)));
    }

    #[test]
    fn parse_compile_log_returns_last_summary() {
        let log = "Result: 1 errors, 0 warnings\nResult: 0 errors, 0 warnings\n";
        assert_eq!(parse_compile_log(log), Some((0, 0)));
    }

    #[test]
    fn validate_rejects_missing_file() {
        let result = validate_mq5_file(Path::new("/nonexistent/file.mq5"));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn validate_rejects_wrong_extension() {
        let tmp = std::env::temp_dir().join("rustmt5_test_wrong_ext.txt");
        std::fs::write(&tmp, "test").unwrap();
        let result = validate_mq5_file(&tmp);
        assert!(matches!(result, Err(Error::InvalidExtension { expected: ".mq5", .. })));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn validate_accepts_mq5_case_insensitive() {
        let tmp = std::env::temp_dir().join("rustmt5_test_case.MQ5");
        std::fs::write(&tmp, "test").unwrap();
        assert!(validate_mq5_file(&tmp).is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn validate_rejects_no_extension() {
        let tmp = std::env::temp_dir().join("rustmt5_test_no_ext");
        std::fs::write(&tmp, "test").unwrap();
        let result = validate_mq5_file(&tmp);
        assert!(matches!(result, Err(Error::InvalidExtension { got: None, .. })));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn validate_output_dir_rejects_missing() {
        let result = validate_output_dir(Path::new("/nonexistent/dir"));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn validate_output_dir_rejects_file() {
        let tmp = std::env::temp_dir().join("rustmt5_test_not_dir");
        std::fs::write(&tmp, "test").unwrap();
        let result = validate_output_dir(&tmp);
        assert!(matches!(result, Err(Error::InvalidOutputDir { .. })));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn validate_output_dir_accepts_directory() {
        let dir = std::env::temp_dir();
        assert!(validate_output_dir(&dir).is_ok());
    }

    #[test]
    fn contains_errors_detects_nonzero() {
        assert!(contains_compile_errors("MyEA.mq5 : 2 error(s), 0 warning(s)"));
    }

    #[test]
    fn contains_errors_ignores_zero() {
        assert!(!contains_compile_errors("MyEA.mq5 : 0 error(s), 3 warning(s)"));
    }

    #[test]
    fn contains_errors_handles_empty() {
        assert!(!contains_compile_errors(""));
    }

    #[test]
    fn contains_errors_handles_no_match() {
        assert!(!contains_compile_errors("Compiling MyEA.mq5\nDone."));
    }

    #[test]
    fn contains_errors_multiline() {
        let output = "MyEA.mq5(5,3) : error 182: 'Prnt' - undeclared identifier\n\
                       MyEA.mq5 : 1 error(s), 0 warning(s)";
        assert!(contains_compile_errors(output));
    }

    #[test]
    fn validate_rejects_empty_path() {
        let result = validate_mq5_file(Path::new(""));
        assert!(matches!(result, Err(Error::FileNotFound { .. })));
    }

    #[test]
    fn validate_mq5_with_dots_in_name() {
        let tmp = std::env::temp_dir().join("my.ea.v2.mq5");
        std::fs::write(&tmp, "test").unwrap();
        assert!(validate_mq5_file(&tmp).is_ok());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn validate_rejects_mq4_extension() {
        let tmp = std::env::temp_dir().join("rustmt5_test_mq4.mq4");
        std::fs::write(&tmp, "test").unwrap();
        let result = validate_mq5_file(&tmp);
        assert!(matches!(result, Err(Error::InvalidExtension { .. })));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn contains_errors_ignores_warning_only_lines() {
        assert!(!contains_compile_errors("MyEA.mq5 : 0 error(s), 5 warning(s)"));
    }

    #[test]
    fn contains_errors_with_leading_whitespace() {
        assert!(contains_compile_errors("  MyEA.mq5 : 1 error(s), 0 warning(s)  "));
    }

    #[test]
    fn log_path_for_replaces_extension() {
        let mq5 = Path::new("/tmp/foo/strategy.mq5");
        assert_eq!(log_path_for(mq5), Path::new("/tmp/foo/strategy.log"));
    }
}
