use std::fs;
use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;

pub fn run(file: &Path) -> Result<()> {
    validate_ini_file(file)?;

    let paths = Mt5Paths::discover()?;
    let wine_path = wine::to_wine_path(file)?;

    eprintln!("Launching strategy tester with {}...", file.display());

    let output = Command::new(&paths.wine)
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

fn validate_ini_file(file: &Path) -> Result<()> {
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
