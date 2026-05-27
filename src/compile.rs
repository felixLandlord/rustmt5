use std::path::Path;
use std::process::Command;

use crate::error::{Error, Result};
use crate::mt5::Mt5Paths;
use crate::wine;

pub fn run(file: &Path) -> Result<()> {
    validate_mq5_file(file)?;

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

    // Check for compilation errors in output
    let combined = format!("{stdout}{stderr}");
    if contains_compile_errors(&combined) {
        return Err(Error::CompileFailed {
            detail: "compilation finished with errors (see output above)".into(),
        });
    }

    let ex5 = file.with_extension("ex5");
    if ex5.exists() {
        eprintln!("Success: {}", ex5.display());
    } else {
        eprintln!("Done. Check output for results.");
    }

    Ok(())
}

fn validate_mq5_file(file: &Path) -> Result<()> {
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

fn contains_compile_errors(output: &str) -> bool {
    output.lines().any(|line| {
        // Match lines like "file.mq5 : 1 error(s), 0 warning(s)"
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
