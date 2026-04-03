use anyhow::Result;

use crate::cli::ScanArgs;
use crate::config::{loader::load_config, merge_cli};
use crate::graph::{build_graph, find_dead_candidates, find_dead_cycles, find_test_only};
use crate::graph::reachability::find_reachable;
use crate::output::write_output;
use crate::parse::{cache::ParseCache, parse_all};
use crate::scoring::assemble_findings;
use crate::scoring::git_history::{
    commit_count_90d, deadness_age_days, get_head_sha, open_repo, score_all_git,
};
use crate::walker::{manifest::detect_languages, walk};

pub fn run(args: ScanArgs) -> Result<()> {
    let file_config = load_config(&args.config)?;
    let config = merge_cli(file_config, &args);
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
    let findings = assemble_findings(&graph, &dead, &dead_cycles, &test_only, &git_scores, &config);

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
    write_output(&findings, &config)?;
    Ok(())
}
