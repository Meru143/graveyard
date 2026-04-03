use anyhow::Result;

use crate::cli::LanguagesArgs;

pub fn run(args: LanguagesArgs) -> Result<()> {
    tracing::debug!(path = ?args.path, "languages command initialized");
    Ok(())
}
