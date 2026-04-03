use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::generate;

use crate::cli::{Cli, CompletionsArgs};

pub fn run(args: CompletionsArgs) -> Result<()> {
    let mut command = Cli::command();
    generate(args.shell, &mut command, "graveyard", &mut io::stdout());
    Ok(())
}
