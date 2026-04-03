use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{BaselineArgs, BaselineCommand, ScanArgs};
use crate::output::write_output;

pub fn run(args: BaselineArgs) -> Result<()> {
    match args.command {
        BaselineCommand::Save(save) => {
            let scan_args = scan_args(save.path, args.verbose);
            let config = crate::cli::scan::load_scan_config(&scan_args)?;
            let findings = crate::cli::scan::run_scan(&scan_args, config)?;
            crate::baseline::save_baseline(&findings, &save.output)?;
        }
        BaselineCommand::Diff(diff) => {
            let scan_args = scan_args(diff.path, args.verbose);
            let config = crate::cli::scan::load_scan_config(&scan_args)?;
            let findings = crate::cli::scan::run_scan(&scan_args, config.clone())?;
            let baseline_fqns = crate::baseline::load_baseline(&diff.baseline)?;
            let findings = crate::baseline::diff_findings(findings, baseline_fqns);
            write_output(&findings, &config)?;

            if diff.ci && !findings.is_empty() {
                std::process::exit(1);
            }
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
        no_cache: true,
        cache_dir: None,
        config: PathBuf::from(".graveyard.toml"),
        verbose,
    }
}
