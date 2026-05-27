use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("MT5 installation not found. Searched /Applications and ~/Applications.\nSet RUSTMT5_WINE, RUSTMT5_EDITOR, and RUSTMT5_TERMINAL environment variables to specify paths manually.")]
    Mt5NotFound,

    #[error("file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("expected a {expected} file, got {got:?}")]
    InvalidExtension { expected: &'static str, got: Option<String> },

    #[error("invalid .ini config: {reason}")]
    InvalidIniFile { reason: String },

    #[error("failed to convert path to Wine format: {path}\n  {reason}")]
    WinePathConversion { path: PathBuf, reason: String },

    #[error("compilation failed:\n{detail}")]
    CompileFailed { detail: String },

    #[error("strategy tester failed:\n{detail}")]
    TestFailed { detail: String },

    #[error("{0}")]
    Io(#[from] std::io::Error),
}
