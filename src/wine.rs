use std::path::Path;

use crate::error::{Error, Result};

/// Converts a macOS absolute path to a Wine Z:-drive path.
///
/// Wine maps the Unix root `/` to `Z:\`, so `/Users/foo/bar.mq5`
/// becomes `Z:\Users\foo\bar.mq5`.
pub fn to_wine_path(path: &Path) -> Result<String> {
    let canonical = path.canonicalize().map_err(|e| Error::WinePathConversion {
        path: path.to_path_buf(),
        reason: format!("failed to canonicalize: {e}"),
    })?;

    let unix_path = canonical.to_str().ok_or_else(|| Error::WinePathConversion {
        path: path.to_path_buf(),
        reason: "path contains invalid UTF-8".into(),
    })?;

    if !unix_path.starts_with('/') {
        return Err(Error::WinePathConversion {
            path: path.to_path_buf(),
            reason: "expected an absolute path".into(),
        });
    }

    // Z: maps to /, so /Users/foo becomes Z:\Users\foo
    let wine_path = format!("Z:{}", unix_path.replace('/', "\\"));
    Ok(wine_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn rejects_non_existent_path() {
        let path = PathBuf::from("/nonexistent/path/file.mq5");
        assert!(to_wine_path(&path).is_err());
    }

    #[test]
    fn converts_existing_path() {
        // /tmp always exists on macOS
        let result = to_wine_path(Path::new("/tmp")).unwrap();
        assert!(result.starts_with("Z:\\"));
        assert!(!result.contains('/'));
    }

    #[test]
    fn converts_temp_file() {
        let tmp = std::env::temp_dir().join("rustmt5_wine_test.mq5");
        std::fs::write(&tmp, "test").unwrap();
        let result = to_wine_path(&tmp).unwrap();
        assert!(result.starts_with("Z:\\"));
        assert!(result.ends_with("rustmt5_wine_test.mq5"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn result_has_no_forward_slashes() {
        let tmp = std::env::temp_dir().join("rustmt5_wine_slash.mq5");
        std::fs::write(&tmp, "test").unwrap();
        let result = to_wine_path(&tmp).unwrap();
        assert!(!result.contains('/'));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn handles_path_with_spaces() {
        let dir = std::env::temp_dir().join("rustmt5 space test");
        let _ = std::fs::create_dir(&dir);
        let file = dir.join("test.mq5");
        std::fs::write(&file, "test").unwrap();
        let result = to_wine_path(&file).unwrap();
        assert!(result.contains("rustmt5 space test"));
        assert!(result.starts_with("Z:\\"));
        let _ = std::fs::remove_file(&file);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn handles_symlink_resolution() {
        // canonicalize resolves symlinks, so /tmp -> /private/tmp on macOS
        let tmp = std::env::temp_dir().join("rustmt5_symlink_test.mq5");
        std::fs::write(&tmp, "test").unwrap();
        let result = to_wine_path(&tmp).unwrap();
        // On macOS, /tmp is a symlink to /private/tmp
        assert!(result.starts_with("Z:\\"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn rejects_relative_nonexistent_path() {
        let result = to_wine_path(Path::new("relative/path.mq5"));
        assert!(result.is_err());
    }
}
