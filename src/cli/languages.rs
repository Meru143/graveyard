use anyhow::Result;

use crate::cli::LanguagesArgs;
use crate::config::Config;
use crate::walker::manifest::detect_languages;

pub fn run(args: LanguagesArgs) -> Result<()> {
    let config = Config::default();
    let mut languages = detect_languages(&args.path, &config)
        .into_iter()
        .map(|language| language.as_str())
        .collect::<Vec<_>>();
    languages.sort_unstable();

    for language in languages {
        println!("{language}");
    }

    Ok(())
}
