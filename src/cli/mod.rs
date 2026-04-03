use std::{path::PathBuf, time::Duration};

use anyhow::Result;
use clap::{ArgAction, Args, Parser, Subcommand};
use clap_complete::Shell;
use tracing_subscriber::EnvFilter;

use crate::config::OutputFormat;

pub mod baseline;
pub mod completions;
pub mod languages;
pub mod scan;

#[derive(Debug, Parser)]
#[command(
    name = "graveyard",
    version,
    about = "Scan polyglot repositories for dead code in a single pass."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Scan(ScanArgs),
    Baseline(BaselineArgs),
    Languages(LanguagesArgs),
    Completions(CompletionsArgs),
}

impl Cli {
    pub fn env_filter(&self) -> EnvFilter {
        let verbose = match &self.command {
            Commands::Scan(args) => args.verbose,
            Commands::Baseline(args) => args.verbose(),
            Commands::Languages(args) => args.verbose,
            Commands::Completions(_) => 0,
        };

        match verbose {
            0 => EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("graveyard=warn")),
            1 => EnvFilter::new("graveyard=debug"),
            _ => EnvFilter::new("graveyard=trace"),
        }
    }
}

pub fn init_tracing(filter: EnvFilter) -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    Ok(())
}

#[derive(Debug, Clone, Args)]
pub struct ScanArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long, value_parser = parse_duration_arg)]
    pub min_age: Option<Duration>,
    #[arg(long, value_parser = parse_confidence_arg)]
    pub min_confidence: Option<f64>,
    #[arg(long)]
    pub top: Option<usize>,
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub exclude: Vec<String>,
    #[arg(long)]
    pub ignore_exports: bool,
    #[arg(long, alias = "fail-on-findings")]
    pub ci: bool,
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    #[arg(long)]
    pub no_git: bool,
    #[arg(long)]
    pub no_cache: bool,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long, default_value = ".graveyard.toml")]
    pub config: PathBuf,
    #[arg(long, short, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Clone, Args)]
pub struct BaselineArgs {
    #[arg(long, short, action = ArgAction::Count, global = true)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: BaselineCommand,
}

impl BaselineArgs {
    pub fn verbose(&self) -> u8 {
        self.verbose
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum BaselineCommand {
    Save(BaselineSaveArgs),
    Diff(BaselineDiffArgs),
}

#[derive(Debug, Clone, Args)]
pub struct BaselineSaveArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long, default_value = ".graveyard-baseline.json")]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct BaselineDiffArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub baseline: PathBuf,
    #[arg(long, alias = "fail-on-findings")]
    pub ci: bool,
}

#[derive(Debug, Clone, Args)]
pub struct LanguagesArgs {
    #[arg(default_value = ".")]
    pub path: PathBuf,
    #[arg(long, short, action = ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Debug, Clone, Args)]
pub struct CompletionsArgs {
    pub shell: Shell,
}

pub fn parse_duration_arg(input: &str) -> Result<Duration, String> {
    if let Some(value) = input.strip_suffix("mo") {
        let months = value.parse::<u64>().map_err(|_| invalid_duration(input))?;
        return Ok(Duration::from_secs(months * 30 * 24 * 60 * 60));
    }

    if let Some(value) = input.strip_suffix('y') {
        let years = value.parse::<u64>().map_err(|_| invalid_duration(input))?;
        return Ok(Duration::from_secs(years * 365 * 24 * 60 * 60));
    }

    humantime::parse_duration(input).map_err(|_| invalid_duration(input))
}

fn invalid_duration(input: &str) -> String {
    format!("invalid duration \"{input}\" — use formats like 7d, 30d, 6mo, 1y")
}

fn parse_confidence_arg(input: &str) -> Result<f64, String> {
    let value = input
        .parse::<f64>()
        .map_err(|_| format!("--min-confidence: {input} is out of range [0.0, 1.0]"))?;

    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(format!(
            "--min-confidence: {input} is out of range [0.0, 1.0]"
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use clap::Parser;

    use super::{parse_confidence_arg, parse_duration_arg, Cli, Commands};

    #[test]
    fn parses_month_duration() {
        assert_eq!(
            parse_duration_arg("6mo").expect("duration should parse"),
            Duration::from_secs(180 * 24 * 60 * 60)
        );
    }

    #[test]
    fn parses_year_duration() {
        assert_eq!(
            parse_duration_arg("1y").expect("duration should parse"),
            Duration::from_secs(365 * 24 * 60 * 60)
        );
    }

    #[test]
    fn parses_scan_command_defaults() {
        let cli = Cli::parse_from(["graveyard", "scan"]);
        match cli.command {
            Commands::Scan(args) => {
                assert_eq!(args.top, None);
                assert_eq!(args.min_confidence, None);
                assert_eq!(args.format, None);
            }
            _ => panic!("expected scan command"),
        }
    }

    #[test]
    fn rejects_confidence_outside_range() {
        let error = parse_confidence_arg("1.5").expect_err("value should fail");
        assert!(error.contains("[0.0, 1.0]"));
    }
}
