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
