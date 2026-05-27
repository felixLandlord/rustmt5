mod cli;
mod compile;
mod error;
mod mt5;
mod test;
mod wine;
mod wine_output;

use std::process;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();

    let result = match cli.command {
        cli::Command::Compile { ref file, ref output } => {
            let output_dir = compile::resolve_output_dir(output.clone());
            compile::run(file, output_dir.as_deref())
        }
        cli::Command::Test { ref file } => test::run(file),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
