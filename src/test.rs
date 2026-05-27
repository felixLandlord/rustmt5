use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;

pub fn run(file: &Path) -> Result<()> {
    validate_ini_file(file)?;

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;

    eprintln!("Launching strategy tester with {}...", file.display());

    let output = paths
        .wine_command()
        .arg(&paths.terminal)
        .arg(format!("/config:{wine_path}"))
        .output()
        .map_err(|e| Error::TestFailed {
            detail: format!("failed to launch Wine: {e}"),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }

    if !output.status.success() {
        return Err(Error::TestFailed {
            detail: format!("terminal64 exited with {}", output.status),
        });
    }

    eprintln!("Strategy tester finished. Check MT5 reports directory for results.");
    Ok(())
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
}
