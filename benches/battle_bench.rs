// Criterion benchmark suite: README-facing Ferroni vs Oniguruma reference numbers.
//
// Run: cargo bench --features ffi --bench battle_bench
// Cache-enabled document comparison:
// cargo bench --features ffi,match-cache --bench battle_bench -- scanner_documents
// HTML report: target/criterion/report/index.html
// Pinned external inputs: benches/battle_inputs.toml

mod general_regex;
mod grammar_loader;
mod scanner_css_workload;
mod scanner_documents;

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use regex::bytes::{Regex, RegexBuilder};
use scanner_css_workload::CSS_INPUT;
use std::hint::black_box;
use std::os::raw::c_uint;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi;
use ferroni::oniguruma::{ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE, OnigOptionType, OnigRegion};
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::regset::{OnigRegSetLead, onig_regset_new, onig_regset_search};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::scanner::{OnigString, Scanner, ScannerFindOptions};

fn configure_battle_group(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.sample_size(30);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(4));
}

fn rust_compile(pattern: &[u8], option: OnigOptionType) -> ferroni::regint::RegexType {
    onig_new(pattern, option, &ONIG_ENCODING_UTF8, &OnigSyntaxOniguruma)
        .expect("Rust compile failed")
}

fn rust_search(
    reg: &ferroni::regint::RegexType,
    text: &[u8],
    region: Option<OnigRegion>,
) -> (i32, Option<OnigRegion>) {
    onig_search(
        reg,
        text,
        text.len(),
        0,
        text.len(),
        region,
        ONIG_OPTION_NONE,
    )
}

fn c_compile(pattern: &[u8], option: c_uint) -> ffi::CRegex {
    ffi::CRegex::new(pattern, option).expect("C compile failed")
}

fn regex_compile(pattern: &[u8], case_insensitive: bool) -> Regex {
    let pat = std::str::from_utf8(pattern).expect("pattern is not UTF-8");
    RegexBuilder::new(pat)
        .case_insensitive(case_insensitive)
        .unicode(true)
        .build()
        .expect("regex compile failed")
}

fn assert_same_match(
    rust_reg: &ferroni::regint::RegexType,
    c_reg: &ffi::CRegex,
    text: &[u8],
    label: &str,
) {
    // Validate captures outside timing. Timed searches request no region from
    // either engine, so C does not perform extra capture-output work.
    let (rust_pos, region) = rust_search(rust_reg, text, Some(OnigRegion::new()));
    let mut c_region = ffi::CRegion::new();
    let c_pos = c_reg.search(
        text,
        0,
        text.len(),
        Some(&mut c_region),
        ffi::ONIG_OPTION_NONE,
    );
    assert_eq!(rust_pos, c_pos, "{label}: match positions differ");
    if rust_pos >= 0 {
        let region = region.unwrap();
        let captures: Vec<_> = region
            .beg
            .into_iter()
            .zip(region.end)
            .take(region.num_regs as usize)
            .collect();
        assert_eq!(
            captures,
            c_region.capture_ranges(),
            "{label}: captures differ"
        );
    }
}

fn assert_same_scanner_trace(scanner: &mut Scanner, c_scanner: &ffi::CScanner, text: &str) {
    assert!(
        text.is_ascii(),
        "trace comparison uses coinciding byte/UTF-16 offsets"
    );
    let line = OnigString::new(text);
    let id = next_c_str_cache_id();
    let mut position = 0;
    while position < text.len() {
        let rust_match = scanner
            .find_next_match_utf16(&line, position, ScannerFindOptions::NONE)
            .map(|m| {
                (
                    m.index,
                    m.capture_indices
                        .iter()
                        .map(|c| (c.start, c.end))
                        .collect::<Vec<_>>(),
                )
            });
        let c_match = c_scanner
            .find_next_match(text.as_bytes(), id, position)
            .map(|(index, captures)| {
                (
                    index,
                    captures
                        .into_iter()
                        .map(|(beg, end)| {
                            // Scanner's public API represents unmatched groups as 0..0.
                            if beg >= 0 && end >= beg {
                                (beg as usize, end as usize)
                            } else {
                                (0, 0)
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            });
        assert_eq!(rust_match, c_match, "scanner trace at {position}: {text:?}");
        let Some((_, captures)) = rust_match else {
            break;
        };
        position = captures[0].1.max(position + 1);
    }
}

fn make_log_line(i: usize) -> String {
    format!(
        "2025-06-{:02} {:02}:{:02}:{:02} INFO server[{}] request path=/api/v1/users/{} status=200 duration={}ms\n",
        (i % 28) + 1,
        i % 24,
        i % 60,
        (i * 7) % 60,
        1000 + (i % 50),
        i * 3,
        (i * 13) % 500,
    )
}

fn make_log_text(num_lines: usize) -> Vec<u8> {
    let mut text = String::new();
    for i in 0..num_lines {
        text.push_str(&make_log_line(i));
    }
    text.into_bytes()
}

fn patterns_to_bytes(patterns: &[&str]) -> Vec<Vec<u8>> {
    patterns
        .iter()
        .map(|pattern| pattern.as_bytes().to_vec())
        .collect()
}

fn bench_scanner_highlighting(c: &mut Criterion) {
    let ts_all = grammar_loader::typescript_patterns();
    let ts_patterns: Vec<&str> = ts_all.iter().map(|pattern| pattern.as_str()).collect();
    let ts_count = ts_patterns.len();
    let ts_patterns_bytes = patterns_to_bytes(&ts_patterns);
    let ts_patterns_byte_refs: Vec<&[u8]> = ts_patterns_bytes
        .iter()
        .map(|pattern| pattern.as_slice())
        .collect();

    let css_all = grammar_loader::css_patterns();
    let css_patterns: Vec<&str> = css_all.iter().map(|pattern| pattern.as_str()).collect();
    let css_count = css_patterns.len();
    let css_patterns_bytes = patterns_to_bytes(&css_patterns);
    let css_patterns_byte_refs: Vec<&[u8]> = css_patterns_bytes
        .iter()
        .map(|pattern| pattern.as_slice())
        .collect();

    let rust_all = grammar_loader::rust_patterns();
    let rust_patterns: Vec<&str> = rust_all.iter().map(|pattern| pattern.as_str()).collect();
    let rust_count = rust_patterns.len();
    let rust_patterns_bytes = patterns_to_bytes(&rust_patterns);
    let rust_patterns_byte_refs: Vec<&[u8]> = rust_patterns_bytes
        .iter()
        .map(|pattern| pattern.as_slice())
        .collect();

    let ts_line = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
    let ts_line_bytes = ts_line.as_bytes();
    let ts_onig = OnigString::new(ts_line);
    let ts_line_len = ts_line.encode_utf16().count();

    let css_onig = OnigString::new(CSS_INPUT);
    let css_input_len = CSS_INPUT.encode_utf16().count();
    let css_input_bytes = CSS_INPUT.as_bytes();

    let rust_line =
        "fn main() -> Result<(), Box<dyn std::error::Error>> { let x: Vec<u32> = vec![1, 2, 3]; }";
    let rust_line_bytes = rust_line.as_bytes();
    let rust_onig = OnigString::new(rust_line);
    let rust_line_len = rust_line.encode_utf16().count();

    let mut group = c.benchmark_group("scanner_highlighting");
    configure_battle_group(&mut group);

    {
        let label = format!("ts_{ts_count}_compile_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&ts_patterns)).unwrap();
                black_box(scanner);
            });
        });

        let label = format!("ts_{ts_count}_compile_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = ffi::CScanner::new(black_box(&ts_patterns_byte_refs))
                    .expect("C scanner failed");
                black_box(scanner);
            });
        });
    }

    {
        let mut scanner = Scanner::new(&ts_patterns).unwrap();
        let c_scanner =
            ffi::CScanner::new(&ts_patterns_byte_refs).expect("C scanner create failed");
        assert_same_scanner_trace(&mut scanner, &c_scanner, ts_line);

        let label = format!("ts_{ts_count}_first_match_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m =
                    scanner.find_next_match_utf16(black_box(&ts_onig), 0, ScannerFindOptions::NONE);
                black_box(m);
            });
        });

        let label = format!("ts_{ts_count}_first_match_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(ts_line_bytes), 0, 0);
                black_box(m);
            });
        });
    }

    {
        let mut scanner = Scanner::new(&ts_patterns).unwrap();
        let c_scanner =
            ffi::CScanner::new(&ts_patterns_byte_refs).expect("C scanner create failed");
        assert_same_scanner_trace(&mut scanner, &c_scanner, ts_line);

        let label = format!("ts_{ts_count}_tokenize_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < ts_line_len {
                    match scanner.find_next_match_utf16(
                        black_box(&ts_onig),
                        pos,
                        ScannerFindOptions::NONE,
                    ) {
                        Some(m) => {
                            let end = m.capture_indices[0].end;
                            pos = if end > pos { end } else { pos + 1 };
                            count += 1;
                        }
                        None => break,
                    }
                }
                black_box(count);
            });
        });

        let label = format!("ts_{ts_count}_tokenize_c");
        let content_len = ts_line_bytes.len();
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < content_len {
                    if let Some((_idx, captures)) =
                        c_scanner.find_next_match(black_box(ts_line_bytes), 0, pos)
                    {
                        let end = captures[0].1 as usize;
                        pos = if end > pos { end } else { pos + 1 };
                        count += 1;
                    } else {
                        break;
                    }
                }
                black_box(count);
            });
        });
    }

    {
        let label = format!("css_{css_count}_compile_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&css_patterns)).unwrap();
                black_box(scanner);
            });
        });

        let label = format!("css_{css_count}_compile_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = ffi::CScanner::new(black_box(&css_patterns_byte_refs))
                    .expect("C scanner failed");
                black_box(scanner);
            });
        });
    }

    {
        let mut scanner = Scanner::new(&css_patterns).unwrap();
        let c_scanner =
            ffi::CScanner::new(&css_patterns_byte_refs).expect("C scanner create failed");
        assert_same_scanner_trace(&mut scanner, &c_scanner, CSS_INPUT);

        let label = format!("css_{css_count}_tokenize_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < css_input_len {
                    match scanner.find_next_match_utf16(
                        black_box(&css_onig),
                        pos,
                        ScannerFindOptions::NONE,
                    ) {
                        Some(m) => {
                            let end = m.capture_indices[0].end;
                            pos = if end > pos { end } else { pos + 1 };
                            count += 1;
                        }
                        None => break,
                    }
                }
                black_box(count);
            });
        });

        let label = format!("css_{css_count}_tokenize_c");
        let content_len = css_input_bytes.len();
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < content_len {
                    if let Some((_idx, captures)) =
                        c_scanner.find_next_match(black_box(css_input_bytes), 0, pos)
                    {
                        let end = captures[0].1 as usize;
                        pos = if end > pos { end } else { pos + 1 };
                        count += 1;
                    } else {
                        break;
                    }
                }
                black_box(count);
            });
        });
    }

    {
        let label = format!("rust_{rust_count}_compile_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&rust_patterns)).unwrap();
                black_box(scanner);
            });
        });

        let label = format!("rust_{rust_count}_compile_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = ffi::CScanner::new(black_box(&rust_patterns_byte_refs))
                    .expect("C scanner failed");
                black_box(scanner);
            });
        });
    }

    {
        let mut scanner = Scanner::new(&rust_patterns).unwrap();
        let c_scanner =
            ffi::CScanner::new(&rust_patterns_byte_refs).expect("C scanner create failed");
        assert_same_scanner_trace(&mut scanner, &c_scanner, rust_line);

        let label = format!("rust_{rust_count}_first_match_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&rust_onig),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });

        let label = format!("rust_{rust_count}_first_match_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(rust_line_bytes), 0, 0);
                black_box(m);
            });
        });
    }

    {
        let mut scanner = Scanner::new(&rust_patterns).unwrap();
        let c_scanner =
            ffi::CScanner::new(&rust_patterns_byte_refs).expect("C scanner create failed");
        assert_same_scanner_trace(&mut scanner, &c_scanner, rust_line);

        let label = format!("rust_{rust_count}_tokenize_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < rust_line_len {
                    match scanner.find_next_match_utf16(
                        black_box(&rust_onig),
                        pos,
                        ScannerFindOptions::NONE,
                    ) {
                        Some(m) => {
                            let end = m.capture_indices[0].end;
                            pos = if end > pos { end } else { pos + 1 };
                            count += 1;
                        }
                        None => break,
                    }
                }
                black_box(count);
            });
        });

        let label = format!("rust_{rust_count}_tokenize_c");
        let content_len = rust_line_bytes.len();
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < content_len {
                    if let Some((_idx, captures)) =
                        c_scanner.find_next_match(black_box(rust_line_bytes), 0, pos)
                    {
                        let end = captures[0].1 as usize;
                        pos = if end > pos { end } else { pos + 1 };
                        count += 1;
                    } else {
                        break;
                    }
                }
                black_box(count);
            });
        });
    }

    group.finish();
}

/// Tokenize one line with the Ferroni scanner; returns the token count.
fn tokenize_line_rust(scanner: &mut Scanner, line: &OnigString, line_len: usize) -> u32 {
    let mut pos = 0usize;
    let mut count = 0u32;
    while pos < line_len {
        match scanner.find_next_match_utf16(black_box(line), pos, ScannerFindOptions::NONE) {
            Some(m) => {
                let end = m.capture_indices[0].end;
                pos = if end > pos { end } else { pos + 1 };
                count += 1;
            }
            None => break,
        }
    }
    count
}

/// Tokenize one line with the C scanner; returns the token count.
fn tokenize_line_c(scanner: &ffi::CScanner, line: &[u8], str_cache_id: i32) -> u32 {
    let mut pos = 0usize;
    let mut count = 0u32;
    while pos < line.len() {
        match scanner.find_next_match(black_box(line), str_cache_id, pos) {
            Some((_idx, captures)) => {
                let end = captures[0].1 as usize;
                pos = if end > pos { end } else { pos + 1 };
                count += 1;
            }
            None => break,
        }
    }
    count
}

/// Cache ids handed to the C scanner. Every line of every iteration gets a new
/// one, so no pattern can answer from the result of a previous line.
static NEXT_C_STR_CACHE_ID: AtomicI32 = AtomicI32::new(1);

fn next_c_str_cache_id() -> i32 {
    NEXT_C_STR_CACHE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Cold path: whole documents tokenized line by line, each line handed to the
/// scanner once, the way vscode-textmate and Shiki drive it. Ferroni sees a
/// distinct `OnigString` per line, the C scanner a fresh `str_cache_id` per
/// line, so neither engine serves a line from the previous line's memo.
fn bench_scanner_documents(c: &mut Criterion) {
    let documents: [(&str, Vec<String>, &str); 3] = [
        (
            "ts",
            grammar_loader::typescript_patterns(),
            scanner_documents::TYPESCRIPT_DOCUMENT,
        ),
        ("css", grammar_loader::css_patterns(), CSS_INPUT),
        (
            "rust",
            grammar_loader::rust_patterns(),
            scanner_documents::RUST_DOCUMENT,
        ),
    ];

    let mut group = c.benchmark_group("scanner_documents");
    configure_battle_group(&mut group);

    for (name, grammar, document) in &documents {
        let patterns: Vec<&str> = grammar.iter().map(|pattern| pattern.as_str()).collect();
        let patterns_bytes = patterns_to_bytes(&patterns);
        let patterns_byte_refs: Vec<&[u8]> = patterns_bytes
            .iter()
            .map(|pattern| pattern.as_slice())
            .collect();

        // Each line with the trailing newline vscode-textmate appends. The
        // documents are ASCII, so UTF-16 and byte offsets coincide.
        let lines: Vec<String> = document.lines().map(|line| format!("{line}\n")).collect();
        assert!(document.is_ascii(), "{name}: document must be ASCII");
        let rust_lines: Vec<(OnigString, usize)> = lines
            .iter()
            .map(|line| (OnigString::new(line), line.len()))
            .collect();

        let mut scanner = Scanner::new(&patterns).unwrap();
        let c_scanner = ffi::CScanner::new(&patterns_byte_refs).expect("C scanner create failed");
        for line in &lines {
            assert_same_scanner_trace(&mut scanner, &c_scanner, line);
        }

        let rust_tokens: u32 = rust_lines
            .iter()
            .map(|(line, len)| tokenize_line_rust(&mut scanner, line, *len))
            .sum();
        let c_tokens: u32 = lines
            .iter()
            .map(|line| tokenize_line_c(&c_scanner, line.as_bytes(), next_c_str_cache_id()))
            .sum();
        assert_eq!(rust_tokens, c_tokens, "{name}: token counts differ");

        let prefix = format!("{name}_{}_document_{}_lines", patterns.len(), lines.len());

        group.bench_function(format!("{prefix}_rust"), |b| {
            b.iter(|| {
                let mut count = 0u32;
                for (line, len) in &rust_lines {
                    count += tokenize_line_rust(&mut scanner, line, *len);
                }
                black_box(count);
            });
        });

        group.bench_function(format!("{prefix}_c"), |b| {
            b.iter(|| {
                let mut count = 0u32;
                for line in &lines {
                    count += tokenize_line_c(&c_scanner, line.as_bytes(), next_c_str_cache_id());
                }
                black_box(count);
            });
        });

        #[cfg(feature = "match-cache")]
        {
            let mut cached = Scanner::with_match_cache(
                &patterns,
                &ferroni::scanner::ScannerConfig::default(),
                ferroni::match_cache::MatchCacheConfig::new(),
            )
            .unwrap();
            for line in &lines {
                assert_same_scanner_trace(&mut cached, &c_scanner, line);
            }
            group.bench_function(format!("{prefix}_rust_cached"), |b| {
                b.iter(|| {
                    let mut count = 0u32;
                    for (line, len) in &rust_lines {
                        count += tokenize_line_rust(&mut cached, line, *len);
                    }
                    black_box(count);
                });
            });
        }
    }

    group.finish();
}

fn bench_text_scanning(c: &mut Criterion) {
    let text_10k = make_log_text(100);
    let text_50k = make_log_text(500);

    let mut group = c.benchmark_group("text_scanning");
    configure_battle_group(&mut group);

    let cases: &[(&str, &[u8], &Vec<u8>)] = &[
        ("literal_50k", b"INFO" as &[u8], &text_50k),
        ("no_match_50k", b"CRITICAL_ERROR" as &[u8], &text_50k),
        ("no_match_10k", b"CRITICAL_ERROR" as &[u8], &text_10k),
        (
            "field_extract_50k",
            b"duration=(\\d+)ms" as &[u8],
            &text_50k,
        ),
        (
            "timestamp_50k",
            b"\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}" as &[u8],
            &text_50k,
        ),
    ];

    for (name, pattern, text) in cases {
        let rust_reg = rust_compile(pattern, ONIG_OPTION_NONE);
        let c_reg = c_compile(pattern, ffi::ONIG_OPTION_NONE);
        let regex = regex_compile(pattern, false);

        assert_same_match(&rust_reg, &c_reg, text.as_slice(), name);

        group.bench_with_input(
            BenchmarkId::new("rust", name),
            &text.as_slice(),
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&rust_reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
        group.bench_with_input(BenchmarkId::new("c", name), &text.as_slice(), |b, text| {
            b.iter(|| {
                let pos = c_reg.search(black_box(text), 0, text.len(), None, ffi::ONIG_OPTION_NONE);
                black_box(pos);
            });
        });
        group.bench_with_input(
            BenchmarkId::new("regex", name),
            &text.as_slice(),
            |b, text| {
                b.iter(|| {
                    let m = regex.find(black_box(text));
                    black_box(m);
                });
            },
        );
    }

    {
        let patterns: &[&[u8]] = &[
            b"Error \\d+",
            b"/api/\\w+/\\d+",
            b"\\d{4}-\\d{2}-\\d{2}",
            b"not found",
            b"\\bpage\\b",
        ];
        let text = b"Error 404: page not found at /api/users/42 on 2025-06-15";
        let rust_regs: Vec<Box<ferroni::regint::RegexType>> = patterns
            .iter()
            .map(|pattern| Box::new(rust_compile(pattern, ONIG_OPTION_NONE)))
            .collect();
        let (rust_set, rc) = onig_regset_new(rust_regs);
        assert!(rc == 0, "Rust regset_new failed: {rc}");
        let mut rust_set = rust_set.unwrap();

        let c_regs_owned: Vec<ffi::CRegex> = patterns
            .iter()
            .map(|pattern| c_compile(pattern, ffi::ONIG_OPTION_NONE))
            .collect();
        let c_raw_ptrs: Vec<ffi::OnigRegex> =
            c_regs_owned.iter().map(|regex| regex.raw()).collect();
        for regex in c_regs_owned {
            std::mem::forget(regex);
        }
        let mut c_set = ffi::CRegSet::new(&c_raw_ptrs).expect("C regset_new failed");
        assert_eq!(
            onig_regset_search(
                &mut rust_set,
                text,
                text.len(),
                0,
                text.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE
            ),
            c_set.search(
                text,
                0,
                text.len(),
                ffi::ONIG_REGSET_POSITION_LEAD,
                ffi::ONIG_OPTION_NONE
            ),
            "regset selection and position differ"
        );

        group.bench_function("regset_position_lead_rust", |b| {
            b.iter(|| {
                let (idx, pos) = onig_regset_search(
                    &mut rust_set,
                    black_box(text),
                    text.len(),
                    0,
                    text.len(),
                    OnigRegSetLead::PositionLead,
                    ONIG_OPTION_NONE,
                );
                black_box((idx, pos));
            });
        });

        group.bench_function("regset_position_lead_c", |b| {
            b.iter(|| {
                let (idx, pos) = c_set.search(
                    black_box(text),
                    0,
                    text.len(),
                    ffi::ONIG_REGSET_POSITION_LEAD,
                    ffi::ONIG_OPTION_NONE,
                );
                black_box((idx, pos));
            });
        });
    }

    group.finish();
}

/// (label, pattern, haystack, Ferroni options, Oniguruma options, regex-compatible)
type SinglePatternCase = (
    &'static str,
    &'static [u8],
    &'static [u8],
    OnigOptionType,
    c_uint,
    bool,
);

fn bench_single_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_pattern");
    configure_battle_group(&mut group);

    let cases: &[SinglePatternCase] = &[
        (
            "literal_exact",
            b"lazy dog",
            b"The quick brown fox jumps over the lazy dog near the riverbank",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
        (
            "quantifier_greedy",
            b"a+b+c+",
            b"aaaaabbbbbccccc12345",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
        (
            "lookaround_combined",
            b"(?<=\\$)\\d+(?=\\.)",
            b"price: $42.99 and cost: $10.00 for item",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            false,
        ),
        (
            "unicode_greek",
            b"\\p{Greek}+",
            "Hello Κόσμε Привет 世界 café résumé naïve".as_bytes(),
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
        (
            "backref_simple",
            b"(\\w+) \\1",
            b"the the quick brown fox fox jumped over",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            false,
        ),
        (
            "case_insensitive_phrase",
            b"brown fox",
            b"The Quick BROWN Fox Jumps OVER the Lazy DOG",
            ONIG_OPTION_IGNORECASE,
            ffi::ONIG_OPTION_IGNORECASE,
            true,
        ),
        (
            "alternation_2_branch",
            b"wolf|wolverine",
            b"The wolverine dashed across the frozen tundra at midnight",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
        (
            "alternation_10_branch",
            b"alpha|beta|gamma|delta|epsilon|zeta|eta|theta|iota|wolverine",
            b"The wolverine dashed across the frozen tundra at midnight",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
        (
            "named_capture_date",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
            b"Event on 2025-12-31 at venue, next on 2026-01-15.",
            ONIG_OPTION_NONE,
            ffi::ONIG_OPTION_NONE,
            true,
        ),
    ];

    for (name, pattern, text, rust_option, c_option, regex_compatible) in cases {
        let rust_reg = rust_compile(pattern, *rust_option);
        let c_reg = c_compile(pattern, *c_option);

        assert_same_match(&rust_reg, &c_reg, text, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = onig_search(
                    &rust_reg,
                    black_box(text),
                    text.len(),
                    0,
                    text.len(),
                    None,
                    ONIG_OPTION_NONE,
                );
                black_box(pos);
            });
        });
        group.bench_with_input(BenchmarkId::new("c", name), &text[..], |b, text| {
            b.iter(|| {
                let pos = c_reg.search(black_box(text), 0, text.len(), None, ffi::ONIG_OPTION_NONE);
                black_box(pos);
            });
        });

        if *regex_compatible {
            let regex = regex_compile(pattern, *rust_option == ONIG_OPTION_IGNORECASE);
            group.bench_with_input(BenchmarkId::new("regex", name), &text[..], |b, text| {
                b.iter(|| {
                    let m = regex.find(black_box(text));
                    black_box(m);
                });
            });
        }
    }

    group.finish();
}

fn bench_compilation(c: &mut Criterion) {
    let cases: &[(&str, &[u8], bool)] = &[
        ("literal", b"hello world", true),
        (
            "named_capture",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
            true,
        ),
        ("lookbehind", b"(?<=@)\\w+", false),
    ];

    let mut group = c.benchmark_group("compilation");
    configure_battle_group(&mut group);

    for (name, pattern, regex_compatible) in cases {
        group.bench_with_input(BenchmarkId::new("rust", name), pattern, |b, pattern| {
            b.iter(|| {
                let reg = rust_compile(black_box(pattern), ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
        group.bench_with_input(BenchmarkId::new("c", name), pattern, |b, pattern| {
            b.iter(|| {
                let reg = c_compile(black_box(pattern), ffi::ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
        if *regex_compatible {
            let pattern = std::str::from_utf8(pattern).unwrap();
            group.bench_with_input(BenchmarkId::new("regex", name), pattern, |b, pattern| {
                b.iter(|| {
                    let regex = Regex::new(black_box(pattern)).unwrap();
                    black_box(&regex);
                });
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    general_regex::bench_general_regex,
    general_regex::bench_unicode_classes,
    bench_scanner_highlighting,
    bench_scanner_documents,
    bench_text_scanning,
    bench_single_pattern,
    bench_compilation,
);
criterion_main!(benches);
