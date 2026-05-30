mod cli;
mod compile;
mod error;
mod metrics;
mod mt5;
mod score;
mod test;
mod text_decode;
mod wine;
mod wine_output;

use std::process;

use clap::Parser;

use crate::error::Error;

fn main() {
    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::Command::Compile { ref file, ref output } => {
            let output_dir = compile::resolve_output_dir(output.clone());
            compile::run(file, output_dir.as_deref())
        }
        cli::Command::Test { ref file, ref input } => {
            let report_dest = test::resolve_report_dest(input.clone(), file);
            test::run(file, report_dest.as_deref()).map_err(Error::from)
        }
        cli::Command::Metrics {
            ref file,
            ref output,
            ref append,
        } => metrics::run(file, output.clone(), append.as_deref()).map_err(Error::from),
        cli::Command::Score { ref config, ref metrics } => {
            score::run(config, metrics).map_err(Error::from)
        }
    };

    if let Err(e) = result {
        eprintln!("{e}");
        process::exit(1);
    }
}
