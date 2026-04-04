pub mod loader;

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::cli::ScanArgs;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Sarif,
    Csv,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Config {
    pub min_confidence: f64,
    pub min_age: Option<Duration>,
    pub fail_on_findings: bool,
    pub top: usize,
    pub format: OutputFormat,
    pub output: Option<PathBuf>,
    pub exclude: Vec<String>,
    pub ignore_exports: bool,
    pub baseline: Option<PathBuf>,
    pub no_git: bool,
    pub no_cache: bool,
    pub cache_enabled: bool,
    pub cache_dir: PathBuf,
    pub no_color: bool,
    pub scoring: ScoringConfig,
    pub ignore: IgnoreConfig,
    pub languages: Vec<String>,
    pub entry_points: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            min_age: None,
            fail_on_findings: false,
            top: 0,
            format: OutputFormat::Table,
            output: None,
            exclude: Vec::new(),
            ignore_exports: false,
            baseline: None,
            no_git: false,
            no_cache: false,
            cache_enabled: true,
            cache_dir: default_cache_dir(),
            no_color: false,
            scoring: ScoringConfig::default(),
            ignore: IgnoreConfig::default(),
            languages: vec![
                "python".to_string(),
                "javascript".to_string(),
                "typescript".to_string(),
                "go".to_string(),
                "rust".to_string(),
            ],
            entry_points: vec![
                "main".to_string(),
                "__main__".to_string(),
                "app".to_string(),
                "handler".to_string(),
                "create_app".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScoringConfig {
    pub age_weight: f64,
    pub ref_weight: f64,
    pub scope_weight: f64,
    pub churn_weight: f64,
    pub age_max_days: u32,
    pub age_min_days: u32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            age_weight: 0.35,
            ref_weight: 0.30,
            scope_weight: 0.20,
            churn_weight: 0.15,
            age_max_days: 730,
            age_min_days: 7,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct IgnoreConfig {
    pub names: Vec<String>,
    pub files: Vec<String>,
    pub decorators: Vec<String>,
}

pub fn merge_cli(mut config: Config, cli: &ScanArgs) -> Config {
    let defaults = Config::default();

    if let Ok(value) = std::env::var("GRAVEYARD_MIN_CONFIDENCE") {
        if config.min_confidence == defaults.min_confidence {
            if let Ok(parsed) = value.parse::<f64>() {
                if (0.0..=1.0).contains(&parsed) {
                    config.min_confidence = parsed;
                }
            }
        }
    }

    if config.no_color == defaults.no_color {
        config.no_color = env_flag("NO_COLOR") || env_flag("GRAVEYARD_NO_COLOR");
    }

    if let Some(min_age) = cli.min_age {
        config.min_age = Some(min_age);
    }

    if let Some(min_confidence) = cli.min_confidence {
        config.min_confidence = min_confidence;
    }

    if let Some(top) = cli.top {
        config.top = top;
    }

    if let Some(format) = cli.format {
        config.format = format;
    }

    if let Some(output) = &cli.output {
        config.output = Some(output.clone());
    }

    if !cli.exclude.is_empty() {
        config.exclude = cli.exclude.clone();
    }

    if cli.ignore_exports {
        config.ignore_exports = true;
    }

    if cli.ci {
        config.fail_on_findings = true;
    }

    if let Some(baseline) = &cli.baseline {
        config.baseline = Some(baseline.clone());
    }

    if cli.no_git {
        config.no_git = true;
    }

    if cli.no_cache {
        config.no_cache = true;
    }

    if let Some(cache_dir) = &cli.cache_dir {
        config.cache_dir = expand_home(cache_dir);
    } else {
        config.cache_dir = expand_home(&config.cache_dir);
    }

    config
}

pub fn expand_home(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if value == "~" {
        return dirs::home_dir()
            .unwrap_or_else(|| dirs::cache_dir().unwrap_or_else(|| path.to_path_buf()));
    }

    if let Some(stripped) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }

        if let Some(cache_dir) = dirs::cache_dir() {
            return cache_dir;
        }
    }

    path.to_path_buf()
}

fn default_cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("graveyard")
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    use clap::Parser;

    use crate::cli::{Cli, Commands};

    use super::{merge_cli, Config, OutputFormat};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn merge_cli_keeps_existing_values_when_flags_are_omitted() {
        let cli = Cli::parse_from(["graveyard", "scan"]);
        let scan = match cli.command {
            Commands::Scan(args) => args,
            _ => panic!("expected scan args"),
        };

        let base = Config {
            min_confidence: 0.8,
            top: 10,
            format: OutputFormat::Json,
            cache_dir: "C:/cache".into(),
            ..Config::default()
        };

        let merged = merge_cli(base.clone(), &scan);

        assert_eq!(merged.min_confidence, base.min_confidence);
        assert_eq!(merged.top, base.top);
        assert_eq!(merged.format, base.format);
        assert_eq!(merged.cache_dir, base.cache_dir);
    }

    #[test]
    fn merge_cli_keeps_toml_values_over_env_defaults() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        std::env::set_var("GRAVEYARD_MIN_CONFIDENCE", "0.9");

        let cli = Cli::parse_from(["graveyard", "scan"]);
        let scan = match cli.command {
            Commands::Scan(args) => args,
            _ => panic!("expected scan args"),
        };

        let base = Config {
            min_confidence: 0.6,
            ..Config::default()
        };

        let merged = merge_cli(base.clone(), &scan);

        assert_eq!(merged.min_confidence, 0.6);
        std::env::remove_var("GRAVEYARD_MIN_CONFIDENCE");
    }

    #[test]
    fn merge_cli_applies_explicit_cli_and_env_overrides() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        std::env::set_var("GRAVEYARD_MIN_CONFIDENCE", "0.9");
        std::env::set_var("NO_COLOR", "1");

        let cli = Cli::parse_from([
            "graveyard",
            "scan",
            "--min-age",
            "7d",
            "--top",
            "5",
            "--format",
            "csv",
            "--cache-dir",
            "~/.cache/graveyard-test",
        ]);

        let scan = match cli.command {
            Commands::Scan(args) => args,
            _ => panic!("expected scan args"),
        };

        let merged = merge_cli(Config::default(), &scan);

        assert_eq!(merged.min_confidence, 0.9);
        assert_eq!(merged.min_age, Some(Duration::from_secs(7 * 24 * 60 * 60)));
        assert_eq!(merged.top, 5);
        assert_eq!(merged.format, OutputFormat::Csv);
        assert!(merged.no_color);
        assert_ne!(
            merged.cache_dir.to_string_lossy(),
            "~/.cache/graveyard-test"
        );

        std::env::remove_var("GRAVEYARD_MIN_CONFIDENCE");
        std::env::remove_var("NO_COLOR");
    }
}
