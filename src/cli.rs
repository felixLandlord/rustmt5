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
    },

    /// Run the MT5 strategy tester with a configuration file
    Test {
        /// Path to the .ini configuration file
        file: PathBuf,
    },
}
