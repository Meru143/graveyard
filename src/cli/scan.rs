use anyhow::Result;

use crate::cli::ScanArgs;

pub fn run(args: ScanArgs) -> Result<()> {
    tracing::debug!(path = ?args.path, "scan command initialized");
    Ok(())
}
