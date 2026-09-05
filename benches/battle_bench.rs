// Criterion benchmark suite: README-facing Ferroni vs Oniguruma reference numbers.
//
// Run: cargo bench --features ffi --bench battle_bench
// HTML report: target/criterion/report/index.html
// Pinned external inputs: benches/battle_inputs.toml

mod grammar_loader;
mod scanner_css_workload;

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, BenchmarkId, Criterion,
};
use regex::bytes::{Regex, RegexBuilder};
use scanner_css_workload::CSS_INPUT;
use std::hint::black_box;
use std::os::raw::c_uint;
use std::time::Duration;

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi;
use ferroni::oniguruma::{OnigOptionType, OnigRegion, ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE};
use ferroni::regcomp::onig_new;
use ferroni::regexec::onig_search;
use ferroni::regset::{onig_regset_new, onig_regset_search, OnigRegSetLead};
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

fn assert_same_result(rust_pos: i32, c_pos: i32, label: &str) {
    debug_assert_eq!(
        rust_pos >= 0,
        c_pos >= 0,
        "{label}: match/mismatch disagree (rust={rust_pos}, c={c_pos})"
    );
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
                            let end = m.capture_indices[0].end as usize;
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
                            let end = m.capture_indices[0].end as usize;
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
                            let end = m.capture_indices[0].end as usize;
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

        let (rust_pos, _) = rust_search(&rust_reg, text.as_slice(), None);
        let c_pos = c_reg.search(text.as_slice(), 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(rust_pos, c_pos, name);

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
            let mut region = ffi::CRegion::new();
            b.iter(|| {
                region.clear();
                let pos = c_reg.search(
                    black_box(text),
                    0,
                    text.len(),
                    Some(&mut region),
                    ffi::ONIG_OPTION_NONE,
                );
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

fn bench_single_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_pattern");
    configure_battle_group(&mut group);

    let cases: &[(&str, &[u8], &[u8], OnigOptionType, c_uint, bool)] = &[
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

        let (rust_pos, _) = onig_search(
            &rust_reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(rust_pos, c_pos, name);

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
            let mut region = ffi::CRegion::new();
            b.iter(|| {
                region.clear();
                let pos = c_reg.search(
                    black_box(text),
                    0,
                    text.len(),
                    Some(&mut region),
                    ffi::ONIG_OPTION_NONE,
                );
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
    bench_scanner_highlighting,
    bench_text_scanning,
    bench_single_pattern,
    bench_compilation,
);
criterion_main!(benches);
