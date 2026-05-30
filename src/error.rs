use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "MT5 installation not found.\n\
         Expected Wine at MetaTrader 5.app/Contents/SharedSupport/wine/bin/wine64\n\
         and MT5 binaries under ~/Library/Application Support/net.metaquotes.wine.metatrader5/.\n\
         Set RUSTMT5_WINEPREFIX, RUSTMT5_WINE, RUSTMT5_EDITOR, and RUSTMT5_TERMINAL to override."
    )]
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

    #[error("{0}")]
    Metrics(#[from] crate::metrics::MetricsError),

    #[error("{0}")]
    Score(#[from] crate::score::ScoreError),
}
