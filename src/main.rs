mod cli;
mod compile;
mod error;
mod mt5;
mod test;
mod wine;

use std::process;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::Command::Compile { ref file } => compile::run(file),
        cli::Command::Test { ref file } => test::run(file),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
