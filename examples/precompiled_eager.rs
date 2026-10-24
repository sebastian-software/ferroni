//! Generate a prepared Eager pair, then load it with ordinary Eager fallback.
use ferroni::prelude::*;
use std::{error::Error, path::Path};

const SETS: &[&[&str]] = &[
    &[r"\b[a-z]+\b", r"(\d+)", r"(?<=prefix:)suffix", r"^end$"],
    &[r"(\d+)", r"\b[a-z]+\b", r"(\d+)"],
    &[r"(?<=x)y", r"(a)\1"],
    &[r"\A\z"],
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let [_, command, directory, rest @ ..] = args.as_slice() else {
        return Err("usage: precompiled_eager <generate|load> DIRECTORY [TEXT]".into());
    };
    let directory = Path::new(directory);
    let config = ScannerConfig::default().acceleration(AccelerationMode::Eager);
    let mut cache = ScannerPatternCache::new();
    match command.as_str() {
        "generate" if rest.is_empty() => {
            for patterns in SETS {
                Scanner::with_pattern_cache(patterns, &config, &mut cache)?;
            }
            let prepared = cache.export_precompiled_eager()?;
            std::fs::create_dir_all(directory)?;
            std::fs::write(
                directory.join("patterns.programs"),
                prepared.program_bytes(),
            )?;
            std::fs::write(directory.join("patterns.portable"), prepared.group_bytes())?;
            println!("Wrote {} prepared groups", prepared.len());
        }
        "load" if rest.len() <= 1 => {
            let pair = std::fs::read(directory.join("patterns.programs")).and_then(|programs| {
                std::fs::read(directory.join("patterns.portable")).map(|groups| (programs, groups))
            });
            match pair {
                Ok((programs, groups)) => match PrecompiledEager::from_bytes(programs, groups) {
                    Ok(prepared) => cache.add_precompiled_eager(prepared),
                    Err(error) => eprintln!("Skipping prepared archive: {error}"),
                },
                Err(error) => eprintln!("Prepared archive unavailable: {error}"),
            }
            let mut scanner = Scanner::with_pattern_cache(SETS[0], &config, &mut cache)?;
            let text = rest.first().map_or("42", String::as_str);
            println!(
                "{:?}",
                scanner.find_next_match(text, 0, ScannerFindOptions::NONE)
            );
        }
        _ => return Err("usage: precompiled_eager <generate|load> DIRECTORY [TEXT]".into()),
    }
    Ok(())
}
