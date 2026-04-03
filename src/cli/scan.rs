use anyhow::Result;

use crate::cli::ScanArgs;
use crate::config::{loader::load_config, merge_cli};
use crate::walker::{manifest::detect_languages, walk};

pub fn run(args: ScanArgs) -> Result<()> {
    let file_config = load_config(&args.config)?;
    let config = merge_cli(file_config, &args);
    let languages = detect_languages(&args.path, &config);
    let files = walk(&args.path, &config);

    tracing::debug!(
        path = ?args.path,
        config = ?config,
        detected_languages = ?languages,
        file_count = files.len(),
        "scan command initialized"
    );
    Ok(())
}
