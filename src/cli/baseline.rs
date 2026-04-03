use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{BaselineArgs, BaselineCommand, ScanArgs};

pub fn run(args: BaselineArgs) -> Result<()> {
    let _load_baseline: fn(&std::path::Path) -> anyhow::Result<std::collections::HashSet<String>> =
        crate::baseline::load_baseline;
    let _diff_findings: fn(
        Vec<crate::parse::types::Finding>,
        std::collections::HashSet<String>,
    ) -> Vec<crate::parse::types::Finding> = crate::baseline::diff_findings;

    match args.command {
        BaselineCommand::Save(save) => {
            let scan_args = scan_args(save.path, args.verbose);
            let (_, findings) = crate::cli::scan::collect_findings(&scan_args)?;
            crate::baseline::save_baseline(&findings, &save.output)?;
        }
        BaselineCommand::Diff(diff) => {
            tracing::debug!(path = ?diff.path, baseline = ?diff.baseline, ci = diff.ci, "baseline diff initialized");
        }
    }

    Ok(())
}

fn scan_args(path: PathBuf, verbose: u8) -> ScanArgs {
    ScanArgs {
        path,
        min_age: None,
        min_confidence: None,
        top: None,
        format: None,
        output: None,
        exclude: Vec::new(),
        ignore_exports: false,
        ci: false,
        baseline: None,
        no_git: false,
        no_cache: false,
        cache_dir: None,
        config: PathBuf::from(".graveyard.toml"),
        verbose,
    }
}
