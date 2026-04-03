use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::cli::parse_duration_arg;
use crate::error::ConfigError;

use super::{Config, IgnoreConfig, OutputFormat, ScoringConfig};

pub fn load_config(config_path: &Path) -> Result<Config> {
    let content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Config::default()),
        Err(error) => return Err(error).with_context(|| config_path.display().to_string()),
    };

    let raw: RawConfig = toml::from_str(&content)
        .map_err(|error| format_toml_error(&content, error))?;

    let mut config = Config::default();

    if let Some(graveyard) = raw.graveyard {
        if let Some(min_confidence) = graveyard.min_confidence {
            config.min_confidence = min_confidence;
        }

        if let Some(min_age) = graveyard.min_age {
            config.min_age = Some(Duration::from_secs(
                parse_duration_arg(&min_age)
                    .map_err(|error| ConfigError::new(format!(".graveyard.toml: {error}")))?
                    .as_secs(),
            ));
        }

        if let Some(fail_on_findings) = graveyard.fail_on_findings {
            config.fail_on_findings = fail_on_findings;
        }

        if let Some(top) = graveyard.top {
            config.top = top;
        }

        if let Some(format) = graveyard.format {
            config.format = format;
        }

        if let Some(output) = graveyard.output {
            config.output = Some(output);
        }

        if let Some(exclude) = graveyard.exclude {
            config.exclude = exclude;
        }

        if let Some(ignore_exports) = graveyard.ignore_exports {
            config.ignore_exports = ignore_exports;
        }

        if let Some(baseline) = graveyard.baseline {
            config.baseline = Some(baseline);
        }

        if let Some(no_git) = graveyard.no_git {
            config.no_git = no_git;
        }

        if let Some(no_cache) = graveyard.no_cache {
            config.no_cache = no_cache;
        }
    }

    if let Some(scoring) = raw.scoring {
        config.scoring = ScoringConfig {
            age_weight: scoring.age_weight.unwrap_or(config.scoring.age_weight),
            ref_weight: scoring.ref_weight.unwrap_or(config.scoring.ref_weight),
            scope_weight: scoring.scope_weight.unwrap_or(config.scoring.scope_weight),
            churn_weight: scoring.churn_weight.unwrap_or(config.scoring.churn_weight),
            age_max_days: scoring.age_max_days.unwrap_or(config.scoring.age_max_days),
            age_min_days: scoring.age_min_days.unwrap_or(config.scoring.age_min_days),
        };
    }

    if let Some(ignore) = raw.ignore {
        config.ignore = IgnoreConfig {
            names: ignore.names.unwrap_or_default(),
            files: ignore.files.unwrap_or_default(),
            decorators: ignore.decorators.unwrap_or_default(),
        };
    }

    if let Some(languages) = raw.languages {
        if let Some(enabled) = languages.enabled {
            config.languages = enabled;
        }
    }

    if let Some(entry_points) = raw.entry_points {
        if let Some(names) = entry_points.names {
            config.entry_points = names;
        }
    }

    if let Some(cache) = raw.cache {
        if let Some(enabled) = cache.enabled {
            config.cache_enabled = enabled;
        }

        if let Some(dir) = cache.dir {
            config.cache_dir = dir;
        }
    }

    validate_scoring(&config.scoring)
        .map_err(|message| ConfigError::new(format!(".graveyard.toml: {message}")))?;
    Ok(config)
}

fn validate_scoring(scoring: &ScoringConfig) -> std::result::Result<(), String> {
    let total =
        scoring.age_weight + scoring.ref_weight + scoring.scope_weight + scoring.churn_weight;

    if (total - 1.0).abs() > f64::EPSILON * 10.0 {
        return Err("scoring weights must sum to 1.0".to_string());
    }

    Ok(())
}

fn format_toml_error(content: &str, error: toml::de::Error) -> ConfigError {
    let line = error
        .span()
        .map(|span| line_number(content, span.start))
        .unwrap_or(1);

    ConfigError::new(format!(
        ".graveyard.toml: invalid TOML at line {line}: {}",
        error.message()
    ))
}

fn line_number(content: &str, index: usize) -> usize {
    let safe_index = index.min(content.len());
    content[..safe_index].chars().filter(|character| *character == '\n').count() + 1
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    graveyard: Option<RawGraveyard>,
    scoring: Option<RawScoringConfig>,
    ignore: Option<RawIgnoreConfig>,
    languages: Option<RawLanguagesConfig>,
    entry_points: Option<RawEntryPointsConfig>,
    cache: Option<RawCacheConfig>,
}

#[derive(Debug, Default, Deserialize)]
struct RawGraveyard {
    min_confidence: Option<f64>,
    min_age: Option<String>,
    fail_on_findings: Option<bool>,
    top: Option<usize>,
    format: Option<OutputFormat>,
    output: Option<PathBuf>,
    exclude: Option<Vec<String>>,
    ignore_exports: Option<bool>,
    baseline: Option<PathBuf>,
    no_git: Option<bool>,
    no_cache: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct RawScoringConfig {
    age_weight: Option<f64>,
    ref_weight: Option<f64>,
    scope_weight: Option<f64>,
    churn_weight: Option<f64>,
    age_max_days: Option<u32>,
    age_min_days: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct RawIgnoreConfig {
    names: Option<Vec<String>>,
    files: Option<Vec<String>>,
    decorators: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawLanguagesConfig {
    enabled: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawEntryPointsConfig {
    names: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCacheConfig {
    enabled: Option<bool>,
    dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use tempfile::tempdir;

    use super::load_config;

    #[test]
    fn load_config_returns_defaults_when_file_is_missing() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp.path().join(".graveyard.toml");

        let config = load_config(&path).expect("missing config should use defaults");

        assert_eq!(config.min_confidence, 0.5);
        assert_eq!(config.min_age, None);
        assert_eq!(config.entry_points[0], "main");
    }

    #[test]
    fn load_config_parses_toml_sections() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp.path().join(".graveyard.toml");
        fs::write(
            &path,
            r#"
[graveyard]
min_confidence = 0.6
min_age = "30d"
fail_on_findings = true
top = 25
format = "json"

[scoring]
age_weight = 0.35
ref_weight = 0.30
scope_weight = 0.20
churn_weight = 0.15
age_max_days = 365
age_min_days = 14

[ignore]
names = ["legacy_*"]
files = ["vendor/**"]
decorators = ["@pytest.fixture"]

[languages]
enabled = ["python", "rust"]

[entry_points]
names = ["main", "handler"]

[cache]
enabled = false
dir = "~/.cache/graveyard-test"
"#,
        )
        .expect("config should be written");

        let config = load_config(&path).expect("config should parse");

        assert_eq!(config.min_confidence, 0.6);
        assert_eq!(config.min_age, Some(Duration::from_secs(30 * 24 * 60 * 60)));
        assert!(config.fail_on_findings);
        assert_eq!(config.top, 25);
        assert_eq!(config.languages, vec!["python".to_string(), "rust".to_string()]);
        assert_eq!(
            config.entry_points,
            vec!["main".to_string(), "handler".to_string()]
        );
        assert!(!config.cache_enabled);
        assert_eq!(config.ignore.names, vec!["legacy_*".to_string()]);
    }

    #[test]
    fn load_config_rejects_invalid_weight_sum() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp.path().join(".graveyard.toml");
        fs::write(
            &path,
            r#"
[scoring]
age_weight = 0.5
ref_weight = 0.5
scope_weight = 0.5
churn_weight = 0.5
"#,
        )
        .expect("config should be written");

        let error = load_config(&path).expect_err("weights should fail validation");
        assert!(error.to_string().contains("sum to 1.0"));
    }

    #[test]
    fn load_config_reports_toml_errors_with_filename() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp.path().join(".graveyard.toml");
        fs::write(&path, "[graveyard]\nmin_confidence =\n").expect("config should be written");

        let error = load_config(&path).expect_err("invalid toml should fail");
        assert!(error.to_string().contains(".graveyard.toml"));
    }
}
