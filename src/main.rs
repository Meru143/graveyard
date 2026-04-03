mod baseline;
mod cli;
mod config;
mod graph;
mod output;
mod parse;
mod scoring;
mod walker;

use anyhow::Result;
use clap::Parser;

fn main() {
    if let Err(error) = run() {
        eprintln!("[ERROR] {error:#}");
        std::process::exit(3);
    }
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse();
    cli::init_tracing(cli.env_filter())?;

    match cli.command {
        cli::Commands::Scan(args) => cli::scan::run(args),
        cli::Commands::Baseline(args) => cli::baseline::run(args),
        cli::Commands::Languages(args) => cli::languages::run(args),
        cli::Commands::Completions(args) => cli::completions::run(args),
    }
}
