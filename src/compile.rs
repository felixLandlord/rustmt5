use std::path::Path;
use std::process::Command;

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

    let output = Command::new(&paths.wine)
        .arg(&paths.editor)
        .arg(format!("/compile:{wine_path}"))
        .arg("/log")
        .output()
        .map_err(|e| Error::CompileFailed {
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
        return Err(Error::CompileFailed {
            detail: format!("metaeditor64 exited with {}", output.status),
        });
    }

    let combined = format!("{stdout}{stderr}");
    if contains_compile_errors(&combined) {
        return Err(Error::CompileFailed {
            detail: "compilation finished with errors (see output above)".into(),
        });
    }

    let ex5 = file.with_extension("ex5");

    if let Some(dir) = output_dir {
        if ex5.exists() {
            let dest = dir.join(ex5.file_name().expect("ex5 has a filename"));
            std::fs::copy(&ex5, &dest)?;
            eprintln!("Success: copied to {}", dest.display());
        } else {
            eprintln!("Done. No .ex5 found to copy — check output for results.");
        }
    } else if ex5.exists() {
        eprintln!("Success: {}", ex5.display());
    } else {
        eprintln!("Done. Check output for results.");
    }

    Ok(())
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
}
