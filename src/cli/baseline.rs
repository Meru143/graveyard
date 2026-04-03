use anyhow::Result;

use crate::cli::{BaselineArgs, BaselineCommand};

pub fn run(args: BaselineArgs) -> Result<()> {
    let _save_baseline: fn(&[crate::parse::types::Finding], &std::path::Path) -> anyhow::Result<()> =
        crate::baseline::save_baseline;

    match args.command {
        BaselineCommand::Save(save) => {
            tracing::debug!(path = ?save.path, output = ?save.output, "baseline save initialized");
        }
        BaselineCommand::Diff(diff) => {
            tracing::debug!(path = ?diff.path, baseline = ?diff.baseline, ci = diff.ci, "baseline diff initialized");
        }
    }

    Ok(())
}
