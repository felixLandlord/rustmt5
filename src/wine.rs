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
}
