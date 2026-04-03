use anyhow::Result;

use crate::cli::ScanArgs;
use crate::config::{loader::load_config, merge_cli};

pub fn run(args: ScanArgs) -> Result<()> {
    let file_config = load_config(&args.config)?;
    let config = merge_cli(file_config, &args);

    tracing::debug!(path = ?args.path, config = ?config, "scan command initialized");
    Ok(())
}
