use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rustmt5",
    version,
    about = "Compile MQL5 files, run MT5 strategy tester, extract metrics, and score backtests"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Compile an MQL5 (.mq5) file into an executable (.ex5)
    Compile {
        /// Path to the .mq5 file to compile
        file: PathBuf,

        /// Copy the compiled .ex5 to MT5 Experts (--output) or to DIR (--output DIR)
        #[arg(
            short,
            long,
            num_args = 0..=1,
            default_missing_value = "__DEFAULT_EXPERTS__",
            value_name = "DIR"
        )]
        output: Option<String>,
    },

    /// Run the MT5 strategy tester with a configuration file
    Test {
        /// Path to the .ini configuration file
        file: PathBuf,

        /// Copy reports to output/test/ (--input) or to DIR (--input DIR); created if absent
        #[arg(
            short,
            long,
            num_args = 0..=1,
            default_missing_value = "__INI_DIR__",
            value_name = "DIR"
        )]
        input: Option<String>,
    },

    /// Extract metrics from an MT5 HTML strategy report into JSON
    Metrics {
        /// Path to the .htm report file
        file: PathBuf,

        /// Output JSON path (default: output/metrics/{report_name}.json)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Append to an existing metrics JSON file
        #[arg(long)]
        append: Option<PathBuf>,
    },

    /// Score backtest metrics using a TOML configuration
    Score {
        /// Path to score configuration (.toml)
        config: PathBuf,

        /// Path to extracted metrics JSON
        metrics: PathBuf,
    },
}
