use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rustmt5",
    version,
    about = "Compile MQL5 files and run MT5 strategy tester from the command line"
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

        /// Copy report files to DIR (--input DIR) or next to the .ini (--input); created if absent
        #[arg(
            short,
            long,
            num_args = 0..=1,
            default_missing_value = "__INI_DIR__",
            value_name = "DIR"
        )]
        input: Option<String>,
    },
}
