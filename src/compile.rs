use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;
use crate::wine_output::filter_wine_noise;

/// Sentinel value when `--output` is passed without a directory (clap `default_missing_value`).
pub const OUTPUT_FLAG_DEFAULT: &str = "__DEFAULT_EXPERTS__";

/// Local artifact directory: `output/compile/` next to the `.mq5`.
pub fn compile_artifacts_dir(mq5_file: &Path) -> PathBuf {
    mq5_file
        .parent()
        .unwrap_or(Path::new("."))
        .join("output/compile")
}

/// Resolve `--output` into a copy destination, if any.
pub fn resolve_output_dir(flag: Option<String>) -> Option<PathBuf> {
    match flag {
        None => None,
        Some(s) if s == OUTPUT_FLAG_DEFAULT => Some(Mt5Paths::default_experts_dir()),
        Some(s) => Some(PathBuf::from(s)),
    }
}

pub fn run(file: &Path, output_dir: Option<&Path>) -> Result<()> {
    validate_mq5_file(file)?;

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;

    // MetaEditor writes the .log next to the .mq5 using the canonical path,
    // so resolve it the same way to find the log after compile.
    let canonical_file = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    let log_path = log_path_for(&canonical_file);

    // Remove stale log so we only read a fresh one after compile
    let _ = std::fs::remove_file(&log_path);
    // Also remove any log at the non-canonical path
    let _ = std::fs::remove_file(log_path_for(file));

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
    // MetaEditor writes the log file next to the source, but under Wine the file can
    // appear a moment after the process exits. Wait briefly for it.
    let log_content = wait_and_read_log(file, &log_path);

    // Print the compile log if available, otherwise fall back to
    // stdout/stderr with Wine noise filtered out.
    if let Some(ref log) = log_content {
        if !log.trim().is_empty() {
            // Print to stdout so it becomes the primary output.
            println!("{}", log.trim_end());
        }
    } else {
        let filtered = filter_wine_noise(&stdout);
        if !filtered.is_empty() {
            println!("{filtered}");
        }
        let filtered_err = filter_wine_noise(&stderr);
        if !filtered_err.is_empty() {
            eprintln!("{filtered_err}");
        }
    }

    let result = evaluate_compile_outcome(
        file,
        output_dir,
        log_content.as_deref(),
        &stdout,
        &stderr,
    );

    // Move .ex5 / .log into output/compile/ whether compile succeeded or failed
    if let Err(e) = organize_compile_artifacts(file) {
        eprintln!("warning: could not move compile artifacts: {e}");
    }

    result
}

fn log_path_for(mq5: &Path) -> PathBuf {
    mq5.with_extension("log")
}

fn read_log(path: &Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    decode_text_file(&bytes)
}

fn decode_text_file(bytes: &[u8]) -> Option<String> {
    // MetaEditor commonly writes logs as UTF-16LE with BOM.
    if bytes.starts_with(&[0xFF, 0xFE]) {
        // Interpret as UTF-16LE.
        let mut u16s = Vec::with_capacity((bytes.len().saturating_sub(2)) / 2);
        for chunk in bytes[2..].chunks_exact(2) {
            u16s.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return Some(String::from_utf16_lossy(&u16s));
    }

    // UTF-8 (or ASCII) fallback.
    std::str::from_utf8(bytes).ok().map(|s| s.to_string())
}

fn wait_and_read_log(mq5: &Path, canonical_log_path: &Path) -> Option<String> {
    let relative_log_path = log_path_for(mq5);

    for _ in 0..50 {
        if let Some(content) = read_log(canonical_log_path) {
            if !content.trim().is_empty() {
                return Some(content);
            }
        }
        if let Some(content) = read_log(&relative_log_path) {
            if !content.trim().is_empty() {
                return Some(content);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    read_log(canonical_log_path).or_else(|| read_log(&relative_log_path))
}

fn evaluate_compile_outcome(
    file: &Path,
    output_dir: Option<&Path>,
    log: Option<&str>,
    stdout: &str,
    stderr: &str,
) -> Result<()> {
    // 1. If we have a log file with a parseable result line, trust it
    if let Some(log) = log {
        if let Some((errors, warnings)) = parse_compile_log(log) {
            if errors == 0 {
                return report_success(file, output_dir, warnings);
            }
            return Err(Error::CompileFailed {
                detail: format!(
                    "compilation finished with {errors} error(s), {warnings} warning(s)"
                ),
            });
        }
    }

    // 2. Check terminal output for error summaries
    let combined = format!("{stdout}{stderr}");
    if contains_compile_errors(&combined) {
        return Err(Error::CompileFailed {
            detail: "compilation finished with errors (see output above)".into(),
        });
    }

    // 3. If .ex5 was produced, treat as success
    let ex5 = file.with_extension("ex5");
    if ex5.exists() {
        return report_success(file, output_dir, 0);
    }

    // 4. No log, no ex5, no clear errors — ambiguous
    Err(Error::CompileFailed {
        detail: format!(
            "no .ex5 produced and no compile log found (check {})",
            log_path_for(file).display()
        ),
    })
}

fn report_success(file: &Path, output_dir: Option<&Path>, warnings: u32) -> Result<()> {
    let ex5 = file.with_extension("ex5");

    // Optional: deploy .ex5 to MT5 Experts (or another path) via --output
    if let Some(dir) = output_dir {
        copy_ex5_if_possible(&ex5, dir)?;
    } else if ex5.exists() {
        eprintln!("Success: {}", compile_artifacts_dir(file).join(ex5.file_name().unwrap()).display());
    } else {
        eprintln!("Compile log reports 0 errors.");
    }

    if warnings > 0 {
        eprintln!("Completed with {warnings} warning(s).");
    }

    Ok(())
}

/// Move compile artifacts into `output/compile/` next to the `.mq5`.
fn organize_compile_artifacts(mq5: &Path) -> Result<()> {
    let ex5 = mq5.with_extension("ex5");
    let log = log_path_for(mq5);
    let canonical_log = mq5
        .canonicalize()
        .unwrap_or_else(|_| mq5.to_path_buf())
        .with_extension("log");

    let dest_dir = compile_artifacts_dir(mq5);
    if !dest_dir.exists() {
        eprintln!("Creating output directory: {}", dest_dir.display());
        std::fs::create_dir_all(&dest_dir).map_err(|e| Error::CompileFailed {
            detail: format!("could not create {}: {e}", dest_dir.display()),
        })?;
    }

    let mut moved = false;
    if move_artifact_if_exists(&ex5, &dest_dir)? {
        moved = true;
    }
    if move_artifact_if_exists(&log, &dest_dir)? {
        moved = true;
    }
    if canonical_log != log && move_artifact_if_exists(&canonical_log, &dest_dir)? {
        moved = true;
    }

    if moved {
        eprintln!("Artifacts saved to {}", dest_dir.display());
    }

    Ok(())
}

fn move_artifact_if_exists(src: &Path, dest_dir: &Path) -> Result<bool> {
    if !src.is_file() {
        return Ok(false);
    }
    let name = src.file_name().expect("artifact has a filename");
    let dest = dest_dir.join(name);
    if dest.exists() {
        std::fs::remove_file(&dest)?;
    }
    std::fs::rename(src, &dest).or_else(|_| {
        std::fs::copy(src, &dest)?;
        std::fs::remove_file(src)?;
        Ok::<(), std::io::Error>(())
    })?;
    eprintln!("  {}", dest.display());
    Ok(true)
}

/// Copy `.ex5` into `dir` when it exists. Warn and skip if the directory is missing.
fn copy_ex5_if_possible(ex5: &Path, dir: &Path) -> Result<()> {
    if !ex5.exists() {
        eprintln!("Compile succeeded, but no .ex5 was found to copy.");
        return Ok(());
    }

    if !dir.exists() {
        eprintln!("Creating output directory: {}", dir.display());
        std::fs::create_dir_all(dir).map_err(|e| Error::CompileFailed {
            detail: format!("could not create output directory {}: {e}", dir.display()),
        })?;
    }

    if !dir.is_dir() {
        eprintln!(
            "warning: output path is not a directory, skipping copy: {}",
            dir.display()
        );
        eprintln!("  compiled .ex5 remains at {}", ex5.display());
        return Ok(());
    }

    let dest = dir.join(ex5.file_name().expect("ex5 has a filename"));
    std::fs::copy(ex5, &dest)?;
    eprintln!("Success: copied to {}", dest.display());
    Ok(())
}

/// Parse error and warning counts from a MetaEditor compile log.
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

    const SAMPLE_LOG: &str = "\n\n\
Z:\\Users\\landlord\\examples\\strategy.mq5 : information: compiling\n\
 : information: code generated\n\
Result: 0 errors, 0 warnings, 623 ms elapsed, cpu='X64 Regular'\n";

    const ERROR_LOG: &str = "\n\n\
Z:\\Users\\landlord\\examples\\strategy_error.mq5 : information: compiling\n\
Z:\\Users\\landlord\\examples\\strategy_error.mq5(36,4) : error 256: undeclared identifier 'Prnt'\n\
Z:\\Users\\landlord\\examples\\strategy_error.mq5(36,10) : error 152: 'Hello' - some operator expected\n\
Result: 2 errors, 0 warnings\n";

    #[test]
    fn parse_compile_log_reads_result_line() {
        assert_eq!(parse_compile_log(SAMPLE_LOG), Some((0, 0)));
    }

    #[test]
    fn parse_compile_log_detects_errors() {
        assert_eq!(parse_compile_log(ERROR_LOG), Some((2, 0)));
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
    fn parse_compile_log_returns_none_for_no_summary() {
        assert_eq!(parse_compile_log("just some text\n"), None);
    }

    #[test]
    fn decode_text_file_decodes_utf16le_bom() {
        // "\r\n\r\nResult: 0 errors, 0 warnings\r\n" encoded as UTF-16LE with BOM.
        let s = "\r\n\r\nResult: 0 errors, 0 warnings\r\n";
        let mut bytes = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        let decoded = decode_text_file(&bytes).unwrap();
        assert!(decoded.contains("Result: 0 errors, 0 warnings"));
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
    fn resolve_output_dir_none_when_flag_absent() {
        assert!(resolve_output_dir(None).is_none());
    }

    #[test]
    fn resolve_output_dir_uses_default_experts_sentinel() {
        let dir = resolve_output_dir(Some(OUTPUT_FLAG_DEFAULT.to_string())).unwrap();
        assert!(dir.to_string_lossy().contains("Experts"));
    }

    #[test]
    fn resolve_output_dir_uses_custom_path() {
        let dir = resolve_output_dir(Some("/tmp/my-build".into())).unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/my-build"));
    }

    #[test]
    fn compile_artifacts_dir_is_next_to_mq5() {
        let mq5 = Path::new("/tmp/project/MyEA.mq5");
        assert_eq!(
            compile_artifacts_dir(mq5),
            PathBuf::from("/tmp/project/output/compile")
        );
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
