mod baseline;
mod cli;
mod config;
mod error;
mod graph;
mod output;
mod parse;
mod scoring;
mod walker;

use anyhow::Result;
use clap::error::ErrorKind;
use clap::Parser;

fn main() {
    if let Err(error) = run() {
        eprintln!("[ERROR] {error:#}");
        std::process::exit(error::exit_code(&error));
    }
}

fn run() -> Result<()> {
    let cli = match cli::Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                error.print()?;
                return Ok(());
            }
            _ => return Err(error::UsageError::new(normalize_clap_error(&error)).into()),
        },
    };
    cli::init_tracing(cli.env_filter())?;

    match cli.command {
        cli::Commands::Scan(args) => cli::scan::run(args),
        cli::Commands::Baseline(args) => cli::baseline::run(args),
        cli::Commands::Languages(args) => cli::languages::run(args),
        cli::Commands::Completions(args) => cli::completions::run(args),
    }
}

fn normalize_clap_error(error: &clap::Error) -> String {
    let rendered = error.to_string();

    if let Some(message) = rendered
        .split("for '--min-age <MIN_AGE>': ")
        .nth(1)
        .and_then(|value| value.lines().next())
    {
        return format!("--min-age: {message}");
    }

    if let Some(message) = rendered
        .split("for '--min-confidence <MIN_CONFIDENCE>': ")
        .nth(1)
        .and_then(|value| value.lines().next())
    {
        return message.to_string();
    }

    rendered
        .lines()
        .next()
        .map(|line| line.trim_start_matches("error: ").to_string())
        .unwrap_or_else(|| "invalid command line usage".to_string())
}
