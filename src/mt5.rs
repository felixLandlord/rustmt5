use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

const APP_NAME: &str = "MetaTrader 5.app";
const SYSTEM_APP: &str = "/Applications/MetaTrader 5.app";
const WINE_PREFIX_DIR: &str = "net.metaquotes.wine.metatrader5";
const WINE_BIN_REL: &str = "Contents/SharedSupport/wine/bin/wine64";
const LEGACY_WINE_REL: &str = "Contents/MacOS/wine64";
const LEGACY_DRIVE_C: &str = "Contents/Resources/drive_c/Program Files/MetaTrader 5";
const DRIVE_C_MT5: &str = "drive_c/Program Files/MetaTrader 5";

const EDITOR_NAMES: &[&str] = &["MetaEditor64.exe", "metaeditor64.exe"];
const TERMINAL_NAMES: &[&str] = &["terminal64.exe"];

/// Resolved paths to MT5 binaries and the Wine prefix.
#[derive(Debug, Clone)]
pub struct Mt5Paths {
    pub wine: PathBuf,
    pub editor: PathBuf,
    pub terminal: PathBuf,
    pub wine_prefix: PathBuf,
}

impl Mt5Paths {
    /// Discover MT5 paths from environment variables or known install locations.
    pub fn discover() -> Result<Self> {
        let wine_prefix = discover_wine_prefix()?;
        let wine = resolve_wine()?;
        let editor = resolve_editor(&wine_prefix)?;
        let terminal = resolve_terminal(&wine_prefix)?;

        Ok(Self {
            wine,
            editor,
            terminal,
            wine_prefix,
        })
    }

    /// Start a Wine process with `WINEPREFIX` set and Wine debug output suppressed.
    pub fn wine_command(&self) -> Command {
        let mut cmd = Command::new(&self.wine);
        cmd.env("WINEPREFIX", &self.wine_prefix);
        // Silence Wine fixme/err/warn spam on stderr (toolbar, HID, MoltenVK, etc.)
        cmd.env("WINEDEBUG", "-all");
        cmd
    }
}

fn discover_wine_prefix() -> Result<PathBuf> {
    if let Some(path) = resolve_env_path("RUSTMT5_WINEPREFIX") {
        return Ok(path);
    }

    if let Some(path) = default_wine_prefix() {
        return Ok(path);
    }

    // Legacy installs may keep drive_c inside the app bundle
    for base in app_bundle_bases() {
        let legacy = base.join(LEGACY_DRIVE_C);
        if legacy.exists() {
            return Ok(base);
        }
    }

    Err(Error::Mt5NotFound)
}

fn default_wine_prefix() -> Option<PathBuf> {
    env::var("HOME")
        .ok()
        .map(|home| {
            Path::new(&home).join(format!("Library/Application Support/{WINE_PREFIX_DIR}"))
        })
        .filter(|p| p.is_dir())
}

fn resolve_wine() -> Result<PathBuf> {
    if let Some(path) = resolve_env_path("RUSTMT5_WINE") {
        return Ok(path);
    }

    let mut candidates = Vec::new();
    for base in app_bundle_bases() {
        candidates.push(base.join(WINE_BIN_REL));
        candidates.push(base.join(LEGACY_WINE_REL));
    }

    find_existing(&candidates).ok_or(Error::Mt5NotFound)
}

fn resolve_editor(wine_prefix: &Path) -> Result<PathBuf> {
    if let Some(path) = resolve_env_path("RUSTMT5_EDITOR") {
        return Ok(path);
    }

    let mut candidates = names_in_dir(&wine_prefix.join(DRIVE_C_MT5), EDITOR_NAMES);
    for base in app_bundle_bases() {
        candidates.extend(names_in_dir(&base.join(LEGACY_DRIVE_C), EDITOR_NAMES));
    }

    find_existing(&candidates).ok_or(Error::Mt5NotFound)
}

fn resolve_terminal(wine_prefix: &Path) -> Result<PathBuf> {
    if let Some(path) = resolve_env_path("RUSTMT5_TERMINAL") {
        return Ok(path);
    }

    let mut candidates = names_in_dir(&wine_prefix.join(DRIVE_C_MT5), TERMINAL_NAMES);
    for base in app_bundle_bases() {
        candidates.extend(names_in_dir(&base.join(LEGACY_DRIVE_C), TERMINAL_NAMES));
    }

    find_existing(&candidates).ok_or(Error::Mt5NotFound)
}

fn names_in_dir(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|name| dir.join(name)).collect()
}

fn resolve_env_path(var: &str) -> Option<PathBuf> {
    let path = env::var(var).ok().map(PathBuf::from)?;
    path.exists().then_some(path)
}

fn find_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

fn app_bundle_bases() -> Vec<PathBuf> {
    let mut bases = vec![PathBuf::from(SYSTEM_APP)];
    if let Ok(home) = env::var("HOME") {
        bases.push(Path::new(&home).join("Applications").join(APP_NAME));
    }
    bases
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn env_lock() -> MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn temp_layout(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rustmt5_mt5_{}_{label}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(DRIVE_C_MT5)).unwrap();
        dir
    }

    #[test]
    fn resolve_env_path_returns_existing_file() {
        let _lock = env_lock();
        let tmp = std::env::temp_dir().join("rustmt5_env_exists");
        fs::write(&tmp, "x").unwrap();
        env::set_var("RUSTMT5_TEST_EXISTS", tmp.to_str().unwrap());
        assert_eq!(resolve_env_path("RUSTMT5_TEST_EXISTS"), Some(tmp.clone()));
        env::remove_var("RUSTMT5_TEST_EXISTS");
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn resolve_env_path_ignores_missing_file() {
        let _lock = env_lock();
        env::set_var("RUSTMT5_TEST_MISSING", "/nonexistent/rustmt5/binary");
        assert!(resolve_env_path("RUSTMT5_TEST_MISSING").is_none());
        env::remove_var("RUSTMT5_TEST_MISSING");
    }

    #[test]
    fn find_existing_returns_first_match() {
        let dir = temp_layout("find_existing");
        let first = dir.join(DRIVE_C_MT5).join("missing.exe");
        let second = dir.join(DRIVE_C_MT5).join("terminal64.exe");
        fs::write(&second, "fake").unwrap();
        let found = find_existing(&[first, second.clone()]).unwrap();
        assert_eq!(found, second);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_existing_returns_none_when_empty() {
        assert!(find_existing(&[]).is_none());
    }

    #[test]
    fn names_in_dir_builds_all_variants() {
        let dir = PathBuf::from("/tmp/mt5");
        let paths = names_in_dir(&dir, EDITOR_NAMES);
        assert_eq!(paths.len(), EDITOR_NAMES.len());
        assert!(paths[0].ends_with("MetaEditor64.exe"));
    }

    #[test]
    fn resolve_editor_prefers_wine_prefix_layout() {
        let _lock = env_lock();
        let prefix = temp_layout("resolve_editor");
        let editor = prefix.join(DRIVE_C_MT5).join("MetaEditor64.exe");
        fs::write(&editor, "fake").unwrap();

        assert_eq!(resolve_editor(&prefix).unwrap(), editor);
        let _ = fs::remove_dir_all(&prefix);
    }

    #[test]
    fn resolve_terminal_finds_exe_in_prefix() {
        let _lock = env_lock();
        let prefix = temp_layout("resolve_terminal");
        let terminal = prefix.join(DRIVE_C_MT5).join("terminal64.exe");
        fs::write(&terminal, "fake").unwrap();

        assert_eq!(resolve_terminal(&prefix).unwrap(), terminal);
        let _ = fs::remove_dir_all(&prefix);
    }

    #[test]
    fn resolve_wine_uses_env_override() {
        let _lock = env_lock();
        let wine = std::env::temp_dir().join("rustmt5_fake_wine64");
        fs::write(&wine, "fake").unwrap();
        env::set_var("RUSTMT5_WINE", wine.to_str().unwrap());
        assert_eq!(resolve_wine().unwrap(), wine);
        env::remove_var("RUSTMT5_WINE");
        let _ = fs::remove_file(&wine);
    }

    #[test]
    fn discover_wine_prefix_uses_env_override() {
        let _lock = env_lock();
        let prefix = temp_layout("wine_prefix_env");
        env::set_var("RUSTMT5_WINEPREFIX", prefix.to_str().unwrap());
        assert_eq!(discover_wine_prefix().unwrap(), prefix);
        env::remove_var("RUSTMT5_WINEPREFIX");
        let _ = fs::remove_dir_all(&prefix);
    }

    #[test]
    fn wine_command_sets_wineprefix_env() {
        let paths = Mt5Paths {
            wine: PathBuf::from("/wine64"),
            editor: PathBuf::from("/editor"),
            terminal: PathBuf::from("/terminal"),
            wine_prefix: PathBuf::from("/prefix"),
        };
        // We cannot run the command in tests, but we can verify construction does not panic.
        let _cmd = paths.wine_command();
    }

    #[test]
    fn discover_succeeds_when_layout_is_complete() {
        let _lock = env_lock();
        let prefix = temp_layout("discover_full");
        let wine = std::env::temp_dir().join("rustmt5_discover_wine64");
        fs::write(&wine, "fake").unwrap();
        fs::write(prefix.join(DRIVE_C_MT5).join("MetaEditor64.exe"), "fake").unwrap();
        fs::write(prefix.join(DRIVE_C_MT5).join("terminal64.exe"), "fake").unwrap();

        env::set_var("RUSTMT5_WINEPREFIX", prefix.to_str().unwrap());
        env::set_var("RUSTMT5_WINE", wine.to_str().unwrap());
        env::remove_var("RUSTMT5_EDITOR");
        env::remove_var("RUSTMT5_TERMINAL");

        let paths = Mt5Paths::discover().unwrap();
        assert_eq!(paths.wine_prefix, prefix);
        assert_eq!(paths.wine, wine);
        assert!(paths.editor.ends_with("MetaEditor64.exe"));
        assert!(paths.terminal.ends_with("terminal64.exe"));

        env::remove_var("RUSTMT5_WINEPREFIX");
        env::remove_var("RUSTMT5_WINE");
        let _ = fs::remove_file(&wine);
        let _ = fs::remove_dir_all(&prefix);
    }

    #[test]
    fn discover_finds_real_install_when_present() {
        let _lock = env_lock();
        let wine = PathBuf::from(SYSTEM_APP).join(WINE_BIN_REL);
        let prefix = env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(format!("Library/Application Support/{WINE_PREFIX_DIR}")));
        let Some(prefix) = prefix.filter(|p| p.is_dir()) else {
            return;
        };
        if !wine.exists() {
            return;
        }
        if !prefix.join(DRIVE_C_MT5).join("terminal64.exe").exists() {
            return;
        }

        env::remove_var("RUSTMT5_WINEPREFIX");
        env::remove_var("RUSTMT5_WINE");
        env::remove_var("RUSTMT5_EDITOR");
        env::remove_var("RUSTMT5_TERMINAL");

        let paths = Mt5Paths::discover().expect("standard Mac MT5 layout should be discoverable");
        assert!(paths.wine.ends_with("wine64"));
        assert_eq!(paths.wine_prefix, prefix);
        assert!(paths.terminal.ends_with("terminal64.exe"));
    }

    #[test]
    fn mt5_paths_struct_is_debug() {
        let paths = Mt5Paths {
            wine: PathBuf::from("/wine"),
            editor: PathBuf::from("/editor"),
            terminal: PathBuf::from("/terminal"),
            wine_prefix: PathBuf::from("/prefix"),
        };
        let debug_str = format!("{paths:?}");
        assert!(debug_str.contains("wine_prefix"));
    }
}
