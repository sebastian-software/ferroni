//! Generate a prepared Eager pair, then load it with ordinary Eager fallback.
use ferroni::prelude::*;
use std::{
    error::Error,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const SETS: &[&[&str]] = &[
    &[r"\b[a-z]+\b", r"(\d+)", r"(?<=prefix:)suffix", r"^end$"],
    &[r"(\d+)", r"\b[a-z]+\b", r"(\d+)"],
    &[r"(?<=x)y", r"(a)\1"],
    &[r"\A\z"],
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let [_, command, directory, rest @ ..] = args.as_slice() else {
        return Err(
            "usage: precompiled_eager <generate|export-index|load|load-records> DIRECTORY [TEXT]"
                .into(),
        );
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
            std::fs::write(
                directory.join("patterns.records"),
                prepared.export_record_index()?,
            )?;
            println!(
                "Wrote {} prepared groups and their record index",
                prepared.len()
            );
        }
        "export-index" if rest.is_empty() => {
            let prepared = PrecompiledEager::from_bytes(
                std::fs::read(directory.join("patterns.programs"))?,
                std::fs::read(directory.join("patterns.portable"))?,
            )?;
            std::fs::write(
                directory.join("patterns.records"),
                prepared.export_record_index()?,
            )?;
        }
        "load-records" if rest.len() <= 1 => {
            let records = std::fs::read(directory.join("patterns.records"))
                .map_err(|error| error.to_string())
                .and_then(|index| {
                    PrecompiledEagerRecords::from_index(&index).map_err(|error| error.to_string())
                });
            match records {
                Ok(source) => {
                    for record in SETS[0]
                        .iter()
                        .filter_map(|pattern| source.program(pattern, &config, false))
                        .chain(source.group(SETS[0], &config, false))
                    {
                        if source.contains(&record) {
                            continue;
                        }
                        let loaded = (|| -> Result<(), Box<dyn Error>> {
                            let file = match record.kind() {
                                PrecompiledEagerRecordKind::Program => "patterns.programs",
                                PrecompiledEagerRecordKind::Group => "patterns.portable",
                                _ => return Err(PrecompiledError::UnsupportedFormat.into()),
                            };
                            let mut file = File::open(directory.join(file))?;
                            let range = record.range();
                            file.seek(SeekFrom::Start(range.start as u64))?;
                            let mut bytes = vec![0; range.len()];
                            file.read_exact(&mut bytes)?;
                            source.insert(&record, bytes.into())?;
                            Ok(())
                        })();
                        if let Err(error) = loaded {
                            eprintln!("Skipping prepared record: {error}");
                        }
                    }
                    cache.add_precompiled_eager_records(source);
                }
                Err(error) => eprintln!("Prepared index unavailable: {error}"),
            }
            let mut scanner = Scanner::with_pattern_cache(SETS[0], &config, &mut cache)?;
            let text = rest.first().map_or("42", String::as_str);
            println!(
                "{:?}",
                scanner.find_next_match(text, 0, ScannerFindOptions::NONE)
            );
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
        _ => return Err(
            "usage: precompiled_eager <generate|export-index|load|load-records> DIRECTORY [TEXT]"
                .into(),
        ),
    }
    Ok(())
}
