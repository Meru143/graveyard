use anyhow::Result;

use crate::cli::ScanArgs;
use crate::config::{loader::load_config, merge_cli};
use crate::parse::{cache::ParseCache, parse_all};
use crate::parse::types::{Finding, FindingTag, ScoreBreakdown};
use crate::walker::{manifest::detect_languages, walk};

pub fn run(args: ScanArgs) -> Result<()> {
    let file_config = load_config(&args.config)?;
    let config = merge_cli(file_config, &args);
    let languages = detect_languages(&args.path, &config);
    let files = walk(&args.path, &config);
    let cache = ParseCache::open(&config.cache_dir, config.no_cache || !config.cache_enabled);
    if config.no_cache {
        cache.clear();
    }
    let (symbols, references) = parse_all(&files, &args.path, &cache, "HEAD", &config);
    let supported_tags = [
        FindingTag::Dead,
        FindingTag::ExportedUnused,
        FindingTag::InDeadCycle,
        FindingTag::TestOnly,
    ];
    let placeholder_breakdown = ScoreBreakdown {
        age_factor: 0.0,
        ref_factor: 0.0,
        scope_factor: 0.0,
        churn_factor: 0.0,
    };
    let findings: Vec<Finding> = Vec::new();

    tracing::debug!(
        path = ?args.path,
        config = ?config,
        detected_languages = ?languages,
        file_count = files.len(),
        symbol_count = symbols.len(),
        reference_count = references.len(),
        supported_tag_count = supported_tags.len(),
        placeholder_finding_count = findings.len(),
        placeholder_breakdown = ?placeholder_breakdown,
        "scan command initialized"
    );
    Ok(())
}
