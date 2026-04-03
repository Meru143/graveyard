use std::time::Instant;

use anyhow::Result;

use crate::cli::ScanArgs;
use crate::config::{Config, loader::load_config, merge_cli};
use crate::graph::{build_graph, find_dead_candidates, find_dead_cycles, find_test_only};
use crate::graph::reachability::find_reachable;
use crate::output::write_output;
use crate::parse::{cache::ParseCache, parse_all};
use crate::parse::types::Finding;
use crate::scoring::assemble_findings;
use crate::scoring::git_history::{
    commit_count_90d, deadness_age_days, get_head_sha, open_repo, score_all_git,
};
use crate::walker::{manifest::detect_languages, walk};

pub(crate) fn load_scan_config(args: &ScanArgs) -> Result<Config> {
    let file_config = load_config(&args.config)?;
    Ok(merge_cli(file_config, args))
}

pub fn run_scan(args: &ScanArgs, config: Config) -> Result<Vec<Finding>> {
    let languages = detect_languages(&args.path, &config);
    let files = walk(&args.path, &config);
    let repo = if config.no_git {
        None
    } else {
        open_repo(&args.path)
    };
    let git_head = repo
        .as_ref()
        .map(get_head_sha)
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let cache = ParseCache::open(&config.cache_dir, config.no_cache || !config.cache_enabled);
    if config.no_cache {
        cache.clear();
    }
    let (symbols, references) = parse_all(&files, &args.path, &cache, &git_head, &config);
    let (graph, _) = build_graph(symbols, references);
    let reachable = find_reachable(&graph, &config.entry_points);
    let dead = find_dead_candidates(&graph, &reachable);
    let dead_cycles = find_dead_cycles(&graph, &reachable);
    let test_only = find_test_only(&graph, &dead);
    let dead_symbols = dead
        .iter()
        .map(|index| graph[*index].clone())
        .collect::<Vec<_>>();
    let git_scores = repo
        .as_ref()
        .filter(|_| !config.no_git)
        .map(|repository| score_all_git(&dead_symbols, repository, &args.path))
        .unwrap_or_default();
    let git_preview = repo
        .as_ref()
        .filter(|_| !config.no_git)
        .and_then(|repository| {
            dead_symbols.first().map(|symbol| {
                (
                    deadness_age_days(symbol, repository, &args.path),
                    commit_count_90d(&symbol.file, repository, &args.path),
                )
            })
        });
    let mut findings =
        assemble_findings(&graph, &dead, &dead_cycles, &test_only, &git_scores, &config);

    if let Some(baseline_path) = &config.baseline {
        let baseline_fqns = crate::baseline::load_baseline(baseline_path)?;
        findings = crate::baseline::diff_findings(findings, baseline_fqns);
    }

    tracing::debug!(
        path = ?args.path,
        config = ?config,
        detected_languages = ?languages,
        file_count = files.len(),
        symbol_count = graph.node_count(),
        reference_count = graph.edge_count(),
        reachable_count = reachable.len(),
        dead_count = dead.len(),
        dead_cycle_count = dead_cycles.len(),
        test_only_count = test_only.len(),
        git_head = %git_head,
        git_score_count = git_scores.len(),
        git_preview_age_days = git_preview.map(|(age_days, _)| age_days),
        git_preview_commits_90d = git_preview.map(|(_, commits_90d)| commits_90d),
        finding_count = findings.len(),
        "scan command initialized"
    );
    Ok(findings)
}

pub fn run(args: ScanArgs) -> Result<()> {
    let started_at = Instant::now();
    let config = load_scan_config(&args)?;
    let findings = run_scan(&args, config.clone())?;
    write_output(&findings, &config)?;
    eprintln!("Scan completed in {:.2}s", started_at.elapsed().as_secs_f64());

    if config.fail_on_findings && !findings.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::baseline::save_baseline;
    use crate::config::Config;
    use crate::parse::types::{Finding, FindingTag, ScoreBreakdown, Symbol, SymbolKind};

    use super::{run_scan, ScanArgs};

    #[test]
    fn run_scan_applies_baseline_diff_before_returning_findings() {
        let temp = tempdir().expect("temp dir should be created");
        fs::create_dir_all(temp.path().join("src")).expect("src dir should exist");
        fs::write(
            temp.path().join("Cargo.toml"),
            r#"[package]
name = "fixture"
version = "0.1.0"
edition = "2021"
"#,
        )
        .expect("manifest should be written");
        fs::write(
            temp.path().join("src/main.rs"),
            r#"
fn old_dead() {}

fn main() {}
"#,
        )
        .expect("rust source should be written");

        let baseline_path = temp.path().join(".graveyard-baseline.json");
        save_baseline(
            &[Finding {
                symbol: Symbol {
                    fqn: "src/main.rs::old_dead".to_string(),
                    name: "old_dead".to_string(),
                    kind: SymbolKind::Function,
                    language: "rust".to_string(),
                    file: temp.path().join("src/main.rs"),
                    line_start: 2,
                    line_end: 2,
                    is_exported: false,
                    is_test: false,
                },
                tag: FindingTag::Dead,
                confidence: 0.82,
                deadness_age_days: 120.0,
                in_degree: 0,
                score_breakdown: ScoreBreakdown {
                    age_factor: 1.0,
                    ref_factor: 1.0,
                    scope_factor: 1.0,
                    churn_factor: 0.5,
                },
            }],
            &baseline_path,
        )
        .expect("baseline should be saved");

        let args = ScanArgs {
            path: temp.path().to_path_buf(),
            min_age: None,
            min_confidence: None,
            top: None,
            format: None,
            output: None,
            exclude: Vec::new(),
            ignore_exports: false,
            ci: false,
            baseline: Some(PathBuf::from(".graveyard-baseline.json")),
            no_git: false,
            no_cache: true,
            cache_dir: None,
            config: PathBuf::from(".graveyard.toml"),
            verbose: 0,
        };
        let mut config = Config::default();
        config.baseline = Some(baseline_path);
        config.no_cache = true;

        let findings = run_scan(&args, config).expect("run_scan should succeed");

        assert!(findings.is_empty(), "baseline should suppress known findings");
    }
}
