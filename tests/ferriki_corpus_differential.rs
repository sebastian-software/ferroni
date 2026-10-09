//! The scanner with its DFA pre-filter (ADR-008) against the scanner
//! without it over a TextMate grammar corpus: every rule's pattern list of
//! every grammar JSON in `FERRONI_GRAMMAR_DIR`, driven line by line over
//! the sample files in `FERRONI_SAMPLE_DIR` the way a highlighter drives a
//! scanner (from every match end, with a stable string id per line), with
//! the pre-filter on and off. Ferroni has no tokenizer, so every rule's
//! scanner runs over every line of the samples written in its grammar's
//! language (paired by file extension), and every grammar's scanners run
//! over a short mixed sample drawn from the first lines of every file.
//!
//! The grammars and samples are not part of this repository; Ferriki's
//! checkout carries them (see CONTRIBUTING.md). The test is ignored, needs
//! both directories, and is meant for a release build:
//!
//! ```sh
//! FERRONI_GRAMMAR_DIR=.../grammars FERRONI_SAMPLE_DIR=.../fixtures \
//!   cargo test --release --test ferriki_corpus_differential -- --ignored --nocapture
//! ```

use ferroni::scanner::{Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch};
use std::path::{Path, PathBuf};

#[path = "../benches/grammar_loader.rs"]
mod grammar_loader;

/// Lines of every sample file in the mixed sample the unpaired grammars run
/// over.
const MIXED_LINES_PER_FILE: usize = 2;

/// Sample file extensions and the grammar file stems they are written in
/// (the tm-grammars layout, one `<id>.json` per language).
const EXTENSIONS: &[(&str, &str)] = &[
    ("adb", "ada"),
    ("asm", "asm"),
    ("astro", "astro"),
    ("c", "c"),
    ("cob", "cobol"),
    ("cpp", "cpp"),
    ("cs", "csharp"),
    ("css", "css"),
    ("f90", "fortran-free-form"),
    ("go", "go"),
    ("html", "html"),
    ("java", "java"),
    ("js", "javascript"),
    ("json", "json"),
    ("m", "objective-c"),
    ("md", "markdown"),
    ("mdx", "mdx"),
    ("pas", "pascal"),
    ("php", "php"),
    ("py", "python"),
    ("r", "r"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("scss", "scss"),
    ("sh", "shellscript"),
    ("sql", "sql"),
    ("svelte", "svelte"),
    ("swift", "swift"),
    ("toml", "toml"),
    ("ts", "typescript"),
    ("tsx", "tsx"),
    ("vb", "vb"),
    ("vue", "vue"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
];

fn bounds(found: Option<ScannerMatch>) -> Option<(usize, Vec<(usize, usize)>)> {
    found.map(|m| {
        (
            m.index,
            m.captures().iter().map(|c| (c.start, c.end)).collect(),
        )
    })
}

/// The grammar stem a sample file is written in.
fn grammar_of(sample: &Path) -> Option<&'static str> {
    let extension = sample.extension()?.to_str()?.to_ascii_lowercase();
    EXTENSIONS
        .iter()
        .find(|(ext, _)| *ext == extension)
        .map(|(_, grammar)| *grammar)
}

/// The files of a directory, sorted by name.
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found
}

/// The lines of a sample as a highlighter hands them to a scanner: each
/// with its newline.
fn lines(text: &str) -> Vec<String> {
    text.lines().map(|line| format!("{line}\n")).collect()
}

struct Counts {
    grammars: usize,
    scanners: usize,
    patterns: usize,
    lines: usize,
    calls: u64,
    mismatches: Vec<String>,
}

/// Tokenizes every line with every scanner pair, comparing each call.
fn tokenize(
    name: &str,
    scanners: &mut [(Vec<String>, Scanner, Scanner)],
    lines: &[String],
    next_id: &mut u64,
    counts: &mut Counts,
) {
    for line in lines {
        *next_id += 1;
        let id = *next_id;
        counts.lines += 1;
        for (patterns, plain, filtered) in scanners.iter_mut() {
            let mut at = 0;
            while at <= line.len() {
                counts.calls += 1;
                let want =
                    bounds(plain.find_next_match_with_id(line, id, at, ScannerFindOptions::NONE));
                let got = bounds(filtered.find_next_match_with_id(
                    line,
                    id,
                    at,
                    ScannerFindOptions::NONE,
                ));
                if got != want {
                    counts.mismatches.push(format!(
                        "{name}: {patterns:?} on {line:?} from {at}: with {got:?}, without {want:?}"
                    ));
                    break;
                }
                let Some((_, captures)) = want else {
                    break;
                };
                let end = captures[0].1;
                at = if end > at {
                    end
                } else {
                    let mut next = at + 1;
                    while next < line.len() && !line.is_char_boundary(next) {
                        next += 1;
                    }
                    next
                };
            }
        }
    }
}

/// Every rule scanner of every grammar answers the same with the
/// pre-filter as without over the sample lines of its language and a mixed
/// sample of every file.
#[test]
#[ignore = "needs FERRONI_GRAMMAR_DIR and FERRONI_SAMPLE_DIR; see CONTRIBUTING.md"]
fn ferriki_corpus_answers_alike_with_and_without_the_prefilter() {
    let grammar_dir = PathBuf::from(
        std::env::var_os("FERRONI_GRAMMAR_DIR").expect("FERRONI_GRAMMAR_DIR is not set"),
    );
    let sample_dir = PathBuf::from(
        std::env::var_os("FERRONI_SAMPLE_DIR").expect("FERRONI_SAMPLE_DIR is not set"),
    );
    let samples: Vec<(PathBuf, Vec<String>)> = files(&sample_dir)
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            Some((path, lines(&text)))
        })
        .collect();
    assert!(
        !samples.is_empty(),
        "no samples in {}",
        sample_dir.display()
    );
    let mixed: Vec<String> = samples
        .iter()
        .flat_map(|(_, lines)| {
            lines
                .iter()
                .filter(|line| line.trim().len() > 1)
                .take(MIXED_LINES_PER_FILE)
                .cloned()
        })
        .collect();

    let plain_config = ScannerConfig::default().prefilter(false);
    let filtered_config = ScannerConfig::default();
    let mut counts = Counts {
        grammars: 0,
        scanners: 0,
        patterns: 0,
        lines: 0,
        calls: 0,
        mismatches: Vec::new(),
    };
    let mut next_id = 0u64;
    let started = std::time::Instant::now();
    for grammar_path in files(&grammar_dir) {
        if grammar_path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = grammar_path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let json = std::fs::read_to_string(&grammar_path).unwrap();
        let lists = grammar_loader::rule_pattern_lists(&json);
        if lists.is_empty() {
            continue;
        }
        counts.grammars += 1;
        let mut scanners: Vec<(Vec<String>, Scanner, Scanner)> = lists
            .into_iter()
            .filter_map(|list| {
                let patterns: Vec<&str> = list.iter().map(String::as_str).collect();
                let plain = Scanner::with_config(&patterns, &plain_config).ok()?;
                let filtered = Scanner::with_config(&patterns, &filtered_config).ok()?;
                Some((list, plain, filtered))
            })
            .collect();
        counts.scanners += scanners.len();
        counts.patterns += scanners
            .iter()
            .map(|(list, _, _)| list.len())
            .sum::<usize>();
        let mut paired = 0;
        for (path, lines) in &samples {
            if grammar_of(path) == Some(stem.as_str()) {
                paired += 1;
                let name = format!("{stem} on {}", path.file_name().unwrap().to_string_lossy());
                tokenize(&name, &mut scanners, lines, &mut next_id, &mut counts);
            }
        }
        if paired == 0 {
            tokenize(
                &format!("{stem} on the mixed sample"),
                &mut scanners,
                &mixed,
                &mut next_id,
                &mut counts,
            );
        }
    }
    println!(
        "{} grammars, {} scanners, {} patterns, {} lines, {} calls compared, {} mismatches, in {:.1?}",
        counts.grammars,
        counts.scanners,
        counts.patterns,
        counts.lines,
        counts.calls,
        counts.mismatches.len(),
        started.elapsed()
    );
    assert!(
        counts.grammars > 0,
        "no grammars in {}",
        grammar_dir.display()
    );
    assert!(
        counts.mismatches.is_empty(),
        "{} mismatches, the first of them:\n{}",
        counts.mismatches.len(),
        counts.mismatches[..counts.mismatches.len().min(20)].join("\n")
    );
}
