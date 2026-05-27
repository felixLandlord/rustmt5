use std::env;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const KNOWN_LOCATIONS: &[&str] = &[
    "/Applications/MetaTrader 5.app",
];

const WINE_REL: &str = "Contents/MacOS/wine64";
const EDITOR_REL: &str = "Contents/Resources/drive_c/Program Files/MetaTrader 5/metaeditor64.exe";
const TERMINAL_REL: &str =
    "Contents/Resources/drive_c/Program Files/MetaTrader 5/terminal64.exe";

/// Resolved paths to MT5 binaries needed for compilation and testing.
#[derive(Debug)]
pub struct Mt5Paths {
    pub wine: PathBuf,
    pub editor: PathBuf,
    pub terminal: PathBuf,
}

impl Mt5Paths {
    /// Discover MT5 paths from environment variables or known install locations.
    pub fn discover() -> Result<Self> {
        let wine = resolve_binary("RUSTMT5_WINE", WINE_REL)?;
        let editor = resolve_binary("RUSTMT5_EDITOR", EDITOR_REL)?;
        let terminal = resolve_binary("RUSTMT5_TERMINAL", TERMINAL_REL)?;

        Ok(Self { wine, editor, terminal })
    }
}

fn resolve_binary(env_var: &str, relative: &str) -> Result<PathBuf> {
    if let Ok(val) = env::var(env_var) {
        let path = PathBuf::from(&val);
        if path.exists() {
            return Ok(path);
        }
        return Err(Error::Mt5NotFound);
    }

    // Search known locations, including ~/Applications
    let mut search_dirs: Vec<PathBuf> = KNOWN_LOCATIONS.iter().map(PathBuf::from).collect();
    if let Ok(home) = env::var("HOME") {
        search_dirs.push(Path::new(&home).join("Applications/MetaTrader 5.app"));
    }

    for base in &search_dirs {
        let candidate = base.join(relative);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(Error::Mt5NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_binary_uses_env_var_when_valid() {
        // Point to an existing file
        let tmp = std::env::temp_dir().join("rustmt5_fake_wine");
        std::fs::write(&tmp, "fake").unwrap();

        env::set_var("RUSTMT5_TEST_RESOLVE", tmp.to_str().unwrap());
        let result = resolve_binary("RUSTMT5_TEST_RESOLVE", "irrelevant/path");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tmp);
        env::remove_var("RUSTMT5_TEST_RESOLVE");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn resolve_binary_errors_when_env_points_to_missing_file() {
        env::set_var("RUSTMT5_TEST_MISSING", "/nonexistent/binary");
        let result = resolve_binary("RUSTMT5_TEST_MISSING", "irrelevant");
        assert!(matches!(result, Err(Error::Mt5NotFound)));
        env::remove_var("RUSTMT5_TEST_MISSING");
    }

    #[test]
    fn resolve_binary_errors_when_not_found_anywhere() {
        // Use an env var name that won't be set
        let result = resolve_binary("RUSTMT5_NONEXISTENT_VAR_12345", "no/such/binary");
        assert!(matches!(result, Err(Error::Mt5NotFound)));
    }

    #[test]
    fn mt5_paths_struct_is_debug() {
        let paths = Mt5Paths {
            wine: PathBuf::from("/wine"),
            editor: PathBuf::from("/editor"),
            terminal: PathBuf::from("/terminal"),
        };
        let debug_str = format!("{paths:?}");
        assert!(debug_str.contains("wine"));
        assert!(debug_str.contains("editor"));
        assert!(debug_str.contains("terminal"));
    }
}
