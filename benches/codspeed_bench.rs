// CodSpeed benchmark suite: Ferroni (Rust) pure performance tracking
//
// Run locally: cargo codspeed build -m simulation && cargo codspeed run
// Or via codspeed CLI: codspeed run --mode simulation -- cargo codspeed run

mod grammar_loader;
mod scanner_css_workload;

use criterion_codspeed::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use scanner_css_workload::{CSS_INPUT, CSS_PATTERNS};

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::{OnigOptionType, OnigRegion, ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{onig_match, onig_region_new, onig_search};
use ferroni::regset::{onig_regset_new, onig_regset_search, OnigRegSetLead};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::scanner::{OnigString, Scanner, ScannerFindOptions};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ===========================================================================
// Tier 2: Regression benchmarks (per-feature coverage, CodSpeed tracking)
// ===========================================================================

// ---------------------------------------------------------------------------
// regression: compile -- measure compilation time
// ---------------------------------------------------------------------------

fn bench_regression_compile(c: &mut Criterion) {
    let patterns: &[(&str, &[u8])] = &[
        ("literal", b"hello world"),
        ("dot_star", b"foo.*bar"),
        ("alternation", b"alpha|beta|gamma|delta"),
        ("char_class", b"[a-zA-Z0-9_]+"),
        ("quantifier", b"a{2,5}b+c?d*"),
        ("group", b"(abc)+(def)*"),
        ("backref", b"(\\w+)\\s+\\1"),
        ("lookahead", b"foo(?=bar)"),
        ("lookbehind", b"(?<=@)\\w+"),
        (
            "named_capture",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
        ),
    ];

    let mut group = c.benchmark_group("regression_compile");
    for (name, pat) in patterns {
        group.bench_with_input(BenchmarkId::new("rust", name), pat, |b, pat| {
            b.iter(|| {
                let reg = rust_compile(black_box(pat), ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: literal_match -- BMH fast-path
// ---------------------------------------------------------------------------

fn bench_regression_literal(c: &mut Criterion) {
    let text = b"The quick brown fox jumps over the lazy dog near the riverbank";
    let cases: &[(&str, &[u8])] = &[
        ("exact", b"lazy dog"),
        ("anchored_start", b"^The quick"),
        ("anchored_end", b"riverbank$"),
        ("word_boundary", b"\\bfox\\b"),
    ];

    let mut group = c.benchmark_group("regression_literal");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: quantifiers
// ---------------------------------------------------------------------------

fn bench_regression_quantifiers(c: &mut Criterion) {
    let text = b"aaaaabbbbbccccc12345";
    let cases: &[(&str, &[u8])] = &[
        ("greedy", b"a+b+c+"),
        ("lazy", b"a+?b+?c+?"),
        ("possessive", b"a++b++"),
        ("nested", b"(a+b+)+"),
    ];

    let mut group = c.benchmark_group("regression_quantifiers");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: alternation
// ---------------------------------------------------------------------------

fn bench_regression_alternation(c: &mut Criterion) {
    let text = b"The wolverine dashed across the frozen tundra at midnight";
    let cases: &[(&str, &[u8])] = &[
        ("two", b"wolf|wolverine"),
        ("five", b"cat|dog|fox|bear|wolverine"),
        (
            "ten",
            b"alpha|beta|gamma|delta|epsilon|zeta|eta|theta|iota|wolverine",
        ),
        ("nested", b"(cat|dog)|(fox|wolverine)"),
    ];

    let mut group = c.benchmark_group("regression_alternation");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: backreferences
// ---------------------------------------------------------------------------

fn bench_regression_backreferences(c: &mut Criterion) {
    let text = b"the the quick brown fox fox jumped over";
    let cases: &[(&str, &[u8])] = &[
        ("simple", b"(\\w+) \\1"),
        ("nested", b"((\\w+) \\2)"),
        ("named", b"(?<word>\\w+) \\k<word>"),
    ];

    let mut group = c.benchmark_group("regression_backreferences");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: lookaround
// ---------------------------------------------------------------------------

fn bench_regression_lookaround(c: &mut Criterion) {
    let text = b"price: $42.99 and cost: $10.00 for item";
    let cases: &[(&str, &[u8])] = &[
        ("pos_lookahead", b"\\$\\d+(?=\\.)"),
        ("neg_lookahead", b"\\$\\d+(?!\\.)"),
        ("pos_lookbehind", b"(?<=\\$)\\d+"),
        ("neg_lookbehind", b"(?<!\\$)\\d+"),
        ("combined", b"(?<=\\$)\\d+(?=\\.)"),
    ];

    let mut group = c.benchmark_group("regression_lookaround");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: unicode_properties
// ---------------------------------------------------------------------------

fn bench_regression_unicode(c: &mut Criterion) {
    let text = "Hello Κόσμε Привет 世界 café résumé naïve".as_bytes();
    let cases: &[(&str, &[u8])] = &[
        ("upper", b"\\p{Lu}+"),
        ("letter", b"\\p{Letter}+"),
        ("greek", b"\\p{Greek}+"),
        ("cyrillic", b"\\p{Cyrillic}+"),
    ];

    let mut group = c.benchmark_group("regression_unicode");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        group.bench_with_input(BenchmarkId::new("rust", name), text, |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: case_insensitive
// ---------------------------------------------------------------------------

fn bench_regression_case_insensitive(c: &mut Criterion) {
    let text = b"The Quick BROWN Fox Jumps OVER the Lazy DOG";
    let cases: &[(&str, &[u8])] = &[
        ("word", b"quick"),
        ("phrase", b"brown fox"),
        ("alternation", b"quick|lazy|dog"),
    ];

    let mut group = c.benchmark_group("regression_case_insensitive");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_IGNORECASE);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = onig_search(
                    &r_reg,
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: named_captures -- extract date fields
// ---------------------------------------------------------------------------

fn bench_regression_named_captures(c: &mut Criterion) {
    let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";
    let pat = b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

    let mut group = c.benchmark_group("regression_named_captures");

    group.bench_function("rust", |b| {
        let mut region = Some(onig_region_new());
        b.iter(|| {
            let mut r = region.take().unwrap();
            r.clear();
            let (pos, returned) = onig_search(
                &r_reg,
                black_box(text),
                text.len(),
                0,
                text.len(),
                Some(r),
                ONIG_OPTION_NONE,
            );
            region = returned;
            black_box(pos);
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// regression: large_text -- realistic log scanning
// ---------------------------------------------------------------------------

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

fn bench_regression_large_text(c: &mut Criterion) {
    let text_10k = make_log_text(100); // ~10KB
    let text_50k = make_log_text(500); // ~50KB

    let cases: &[(&str, &[u8])] = &[
        ("literal_INFO", b"INFO"),
        ("timestamp", b"\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}"),
        ("field_extract", b"duration=(\\d+)ms"),
        ("no_match", b"CRITICAL_ERROR"),
    ];

    let mut group = c.benchmark_group("regression_large_text");

    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

        // 10KB
        let label_10k = format!("{}_10k", name);
        group.bench_with_input(
            BenchmarkId::new("rust", &label_10k),
            &text_10k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&r_reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );

        // 50KB
        let label_50k = format!("{}_50k", name);
        group.bench_with_input(
            BenchmarkId::new("rust", &label_50k),
            &text_50k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&r_reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: regset -- multi-pattern matching
// ---------------------------------------------------------------------------

fn bench_regression_regset(c: &mut Criterion) {
    let text = b"Error 404: page not found at /api/users/42 on 2025-06-15";

    let patterns: &[&[u8]] = &[
        b"Error \\d+",
        b"/api/\\w+/\\d+",
        b"\\d{4}-\\d{2}-\\d{2}",
        b"not found",
        b"\\bpage\\b",
    ];

    let rust_regs: Vec<Box<ferroni::regint::RegexType>> = patterns
        .iter()
        .map(|p| Box::new(rust_compile(p, ONIG_OPTION_NONE)))
        .collect();
    let (rust_set, rc) = onig_regset_new(rust_regs);
    assert!(rc == 0, "Rust regset_new failed: {rc}");
    let mut rust_set = rust_set.unwrap();

    let mut group = c.benchmark_group("regression_regset");

    // Position-lead
    group.bench_function("position_lead", |b| {
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

    // Regex-lead
    group.bench_function("regex_lead", |b| {
        b.iter(|| {
            let (idx, pos) = onig_regset_search(
                &mut rust_set,
                black_box(text),
                text.len(),
                0,
                text.len(),
                OnigRegSetLead::RegexLead,
                ONIG_OPTION_NONE,
            );
            black_box((idx, pos));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// regression: match_at_position -- onig_match at a known offset
// ---------------------------------------------------------------------------

fn bench_regression_match_at_position(c: &mut Criterion) {
    let text = b"xxxx1234abcd";
    let pat = b"\\d+";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

    let mut group = c.benchmark_group("regression_match_at_position");

    group.bench_function("rust", |b| {
        b.iter(|| {
            let (len, _) = onig_match(
                &r_reg,
                black_box(text),
                text.len(),
                4,
                None,
                ONIG_OPTION_NONE,
            );
            black_box(len);
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// regression: scanner -- Scanner API
// ---------------------------------------------------------------------------

const SCANNER_PATTERNS: &[&str] = &[
    "Error \\d+",
    "/api/\\w+/\\d+",
    "\\d{4}-\\d{2}-\\d{2}",
    "not found",
    "\\bpage\\b",
];

const SCANNER_TEXT_SHORT: &str = "Error 404: page not found at /api/users/42 on 2025-06-15";

fn make_long_text() -> Vec<u8> {
    let base = b"Error 404: page not found at /api/users/42 on 2025-06-15. ";
    let mut text = Vec::with_capacity(base.len() * 40);
    for _ in 0..40 {
        text.extend_from_slice(base);
    }
    text
}

fn bench_regression_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_scanner");

    // short_string: Scanner (RegSet fast-path)
    {
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();

        group.bench_function("short_string", |b| {
            b.iter(|| {
                let m = scanner.find_next_match(
                    black_box(SCANNER_TEXT_SHORT),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // long_string_cold: per-regex path, no caching
    {
        let long = make_long_text();
        let long_str = std::str::from_utf8(&long).unwrap();
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();

        group.bench_function("long_string_cold", |b| {
            b.iter(|| {
                let m = scanner.find_next_match(black_box(long_str), 0, ScannerFindOptions::NONE);
                black_box(m);
            });
        });
    }

    // long_string_warm: per-regex path, cache hits
    {
        let long = make_long_text();
        let long_str = std::str::from_utf8(&long).unwrap();
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();

        // Prime the cache
        scanner.find_next_match_with_id(long_str, 1, 0, ScannerFindOptions::NONE);

        group.bench_function("long_string_warm", |b| {
            b.iter(|| {
                let m = scanner.find_next_match_with_id(
                    black_box(long_str),
                    1,
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // utf16: OnigString creation + find_next_match_utf16
    {
        let content = "Error 404: page «not found» at /api/users/42 on 2025-06-15 — résumé";
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();

        group.bench_function("utf16", |b| {
            b.iter(|| {
                let s = OnigString::new(black_box(content));
                let m = scanner.find_next_match_utf16(&s, 0, ScannerFindOptions::NONE);
                black_box(m);
            });
        });
    }

    // css workload: single representative tokenize benchmark
    {
        let patterns: Vec<&str> = CSS_PATTERNS
            .iter()
            .copied()
            .filter(|p| Scanner::new(&[*p]).is_ok())
            .collect();
        let pattern_count = patterns.len();

        let onig_str = OnigString::new(CSS_INPUT);
        let mut scanner = Scanner::new(&patterns).unwrap();
        let input_len = CSS_INPUT.encode_utf16().count();

        let label = format!("css_{pattern_count}_patterns_tokenize");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < input_len {
                    match scanner.find_next_match_utf16(
                        black_box(&onig_str),
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
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// regression: scanner_textmate -- TextMate-realistic Scanner workload (65 patterns)
// ---------------------------------------------------------------------------

fn bench_regression_scanner_textmate(c: &mut Criterion) {
    let ts_all = grammar_loader::typescript_patterns();
    let patterns: Vec<&str> = ts_all.iter().map(|s| s.as_str()).collect();
    let pattern_count = patterns.len();
    let mut group = c.benchmark_group("regression_scanner_textmate");

    // compile: measure Scanner::new compilation cost for N real TS patterns
    {
        let label = format!("compile_{pattern_count}_patterns");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // match_short: single TS line, match from position 0 (fast-path if RegSet kicks in)
    {
        let content = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
        let onig_str = OnigString::new(content);
        let mut scanner = Scanner::new(&patterns).unwrap();

        let label = format!("{pattern_count}_patterns_short");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&onig_str),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // match_mid: start scanning from middle of line (many patterns won't match early)
    {
        let content = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
        let onig_str = OnigString::new(content);
        let mut scanner = Scanner::new(&patterns).unwrap();
        // Start at UTF-16 position 34 (after "fetchUsers({ ")
        let start_pos = 34;

        let label = format!("{pattern_count}_patterns_mid_offset");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&onig_str),
                    black_box(start_pos),
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // match_long: ~7KB repeated TypeScript input (per-regex path)
    {
        let line = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call\n";
        let content: String = line.repeat(100);
        let onig_str = OnigString::new(&content);
        let mut scanner = Scanner::new(&patterns).unwrap();

        let label = format!("{pattern_count}_patterns_long");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&onig_str),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // tokenize_loop: simulate real tokenizer -- scan entire line token-by-token
    {
        let content = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
        let onig_str = OnigString::new(content);
        let mut scanner = Scanner::new(&patterns).unwrap();
        let line_len = content.encode_utf16().count();

        let label = format!("{pattern_count}_patterns_tokenize_line");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < line_len {
                    match scanner.find_next_match_utf16(
                        black_box(&onig_str),
                        pos,
                        ScannerFindOptions::NONE,
                    ) {
                        Some(m) => {
                            let end = m.capture_indices[0].end as usize;
                            // Advance at least 1 position to avoid infinite loops
                            pos = if end > pos { end } else { pos + 1 };
                            count += 1;
                        }
                        None => break,
                    }
                }
                black_box(count);
            });
        });
    }

    // 5_patterns_short: baseline comparison with existing 5-pattern set
    {
        let content = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
        let onig_str = OnigString::new(content);
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();

        group.bench_function("5_patterns_short", |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&onig_str),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// regression: idiomatic API -- Regex::new / find / captures
// ---------------------------------------------------------------------------

fn bench_regression_idiomatic_api(c: &mut Criterion) {
    use ferroni::prelude::*;

    let mut group = c.benchmark_group("regression_idiomatic_api");

    // compile
    group.bench_function("compile", |b| {
        b.iter(|| {
            let re = Regex::new(black_box(r"\d{4}-\d{2}-\d{2}")).unwrap();
            black_box(&re);
        });
    });

    // find
    {
        let re = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
        let text = "Date: 2026-02-12 and 2026-03-15";
        group.bench_function("find", |b| {
            b.iter(|| {
                let m = re.find(black_box(text));
                black_box(m);
            });
        });
    }

    // is_match
    {
        let re = Regex::new(r"\b(function|const|let|var)\b").unwrap();
        let text = "const x = 42; let y = function() { return x; };";
        group.bench_function("is_match", |b| {
            b.iter(|| {
                let m = re.is_match(black_box(text));
                black_box(m);
            });
        });
    }

    // captures
    {
        let re = Regex::new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})").unwrap();
        let text = "Event on 2025-12-31 at venue";
        group.bench_function("captures", |b| {
            b.iter(|| {
                let caps = re.captures(black_box(text));
                black_box(caps);
            });
        });
    }

    group.finish();
}

// ===========================================================================
// Tier 1: Real-world scenario benchmarks (README-facing)
// ===========================================================================

// ---------------------------------------------------------------------------
// scanner_highlighting -- the Shiki / VS Code / TextMate workload
// ---------------------------------------------------------------------------

fn bench_scanner_highlighting(c: &mut Criterion) {
    // Load full, unmodified Shiki grammars
    let ts_all = grammar_loader::typescript_patterns();
    let ts_patterns: Vec<&str> = ts_all.iter().map(|s| s.as_str()).collect();
    let ts_count = ts_patterns.len();

    let css_all = grammar_loader::css_patterns();
    let css_patterns: Vec<&str> = css_all.iter().map(|s| s.as_str()).collect();
    let css_count = css_patterns.len();

    let rust_all = grammar_loader::rust_patterns();
    let rust_patterns: Vec<&str> = rust_all.iter().map(|s| s.as_str()).collect();
    let rust_count = rust_patterns.len();

    let ts_line = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
    let ts_onig = OnigString::new(ts_line);
    let ts_line_len = ts_line.encode_utf16().count();

    let css_onig = OnigString::new(CSS_INPUT);
    let css_input_len = CSS_INPUT.encode_utf16().count();

    let rust_line = "fn main() -> Result<(), Box<dyn std::error::Error>> { let x: Vec<u32> = vec![1, 2, 3]; }";
    let rust_onig = OnigString::new(rust_line);
    let rust_line_len = rust_line.encode_utf16().count();

    let mut group = c.benchmark_group("scanner_highlighting");

    // -- TypeScript: compile --
    {
        let label = format!("ts_{ts_count}_patterns_compile");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&ts_patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // -- TypeScript: first match --
    {
        let mut scanner = Scanner::new(&ts_patterns).unwrap();
        let label = format!("ts_{ts_count}_patterns_first_match");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = scanner.find_next_match_utf16(
                    black_box(&ts_onig),
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    // -- TypeScript: tokenize line --
    {
        let mut scanner = Scanner::new(&ts_patterns).unwrap();
        let label = format!("ts_{ts_count}_patterns_tokenize");
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
    }

    // -- CSS: compile --
    {
        let label = format!("css_{css_count}_patterns_compile");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&css_patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // -- CSS: tokenize --
    {
        let mut scanner = Scanner::new(&css_patterns).unwrap();
        let label = format!("css_{css_count}_patterns_tokenize");
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
    }

    // -- Rust: compile --
    {
        let label = format!("rust_{rust_count}_patterns_compile");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&rust_patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // -- Rust: first match --
    {
        let mut scanner = Scanner::new(&rust_patterns).unwrap();
        let label = format!("rust_{rust_count}_patterns_first_match");
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
    }

    // -- Rust: tokenize line --
    {
        let mut scanner = Scanner::new(&rust_patterns).unwrap();
        let label = format!("rust_{rust_count}_patterns_tokenize");
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
    }

    // -- warm cache: scanner with primed cache (steady-state path) --
    {
        let long = make_long_text();
        let long_str = std::str::from_utf8(&long).unwrap();
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();
        scanner.find_next_match_with_id(long_str, 1, 0, ScannerFindOptions::NONE);

        group.bench_function("warm_cache", |b| {
            b.iter(|| {
                let m = scanner.find_next_match_with_id(
                    black_box(long_str),
                    1,
                    0,
                    ScannerFindOptions::NONE,
                );
                black_box(m);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// text_scanning -- large text search: log parsing, data extraction
// ---------------------------------------------------------------------------

fn bench_text_scanning(c: &mut Criterion) {
    let text_10k = make_log_text(100);
    let text_50k = make_log_text(500);

    let mut group = c.benchmark_group("text_scanning");

    // literal 50KB
    {
        let reg = rust_compile(b"INFO", ONIG_OPTION_NONE);
        group.bench_with_input(
            BenchmarkId::new("rust", "literal_50k"),
            &text_50k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }

    // no_match 50KB
    {
        let reg = rust_compile(b"CRITICAL_ERROR", ONIG_OPTION_NONE);
        group.bench_with_input(
            BenchmarkId::new("rust", "no_match_50k"),
            &text_50k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }

    // no_match 10KB
    {
        let reg = rust_compile(b"CRITICAL_ERROR", ONIG_OPTION_NONE);
        group.bench_with_input(
            BenchmarkId::new("rust", "no_match_10k"),
            &text_10k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }

    // field_extract 50KB
    {
        let reg = rust_compile(b"duration=(\\d+)ms", ONIG_OPTION_NONE);
        group.bench_with_input(
            BenchmarkId::new("rust", "field_extract_50k"),
            &text_50k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }

    // timestamp 50KB
    {
        let reg = rust_compile(
            b"\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}",
            ONIG_OPTION_NONE,
        );
        group.bench_with_input(
            BenchmarkId::new("rust", "timestamp_50k"),
            &text_50k,
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&reg, black_box(text), None);
                    black_box(pos);
                });
            },
        );
    }

    // regset position-lead
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
            .map(|p| Box::new(rust_compile(p, ONIG_OPTION_NONE)))
            .collect();
        let (rust_set, rc) = onig_regset_new(rust_regs);
        assert!(rc == 0, "Rust regset_new failed: {rc}");
        let mut rust_set = rust_set.unwrap();

        group.bench_function("regset_position_lead", |b| {
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
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// single_pattern -- one representative per regex feature category
// ---------------------------------------------------------------------------

fn bench_single_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_pattern");

    let cases: &[(&str, &[u8], &[u8], OnigOptionType)] = &[
        (
            "literal_exact",
            b"lazy dog",
            b"The quick brown fox jumps over the lazy dog near the riverbank",
            ONIG_OPTION_NONE,
        ),
        (
            "quantifier_greedy",
            b"a+b+c+",
            b"aaaaabbbbbccccc12345",
            ONIG_OPTION_NONE,
        ),
        (
            "lookaround_combined",
            b"(?<=\\$)\\d+(?=\\.)",
            b"price: $42.99 and cost: $10.00 for item",
            ONIG_OPTION_NONE,
        ),
        (
            "unicode_greek",
            b"\\p{Greek}+",
            "Hello Κόσμε Привет 世界 café résumé naïve".as_bytes(),
            ONIG_OPTION_NONE,
        ),
        (
            "backref_simple",
            b"(\\w+) \\1",
            b"the the quick brown fox fox jumped over",
            ONIG_OPTION_NONE,
        ),
        (
            "case_insensitive_phrase",
            b"brown fox",
            b"The Quick BROWN Fox Jumps OVER the Lazy DOG",
            ONIG_OPTION_IGNORECASE,
        ),
        (
            "alternation_2_branch",
            b"wolf|wolverine",
            b"The wolverine dashed across the frozen tundra at midnight",
            ONIG_OPTION_NONE,
        ),
        (
            "alternation_10_branch",
            b"alpha|beta|gamma|delta|epsilon|zeta|eta|theta|iota|wolverine",
            b"The wolverine dashed across the frozen tundra at midnight",
            ONIG_OPTION_NONE,
        ),
        (
            "named_capture_date",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
            b"Event on 2025-12-31 at venue, next on 2026-01-15.",
            ONIG_OPTION_NONE,
        ),
    ];

    for (name, pat, text, option) in cases {
        let reg = rust_compile(pat, *option);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = onig_search(
                    &reg,
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
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// compilation -- representative compile-time spread
// ---------------------------------------------------------------------------

fn bench_compilation(c: &mut Criterion) {
    let cases: &[(&str, &[u8])] = &[
        ("literal", b"hello world"),
        (
            "named_capture",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
        ),
        ("lookbehind", b"(?<=@)\\w+"),
    ];

    let mut group = c.benchmark_group("compilation");
    for (name, pat) in cases {
        group.bench_with_input(BenchmarkId::new("rust", name), pat, |b, pat| {
            b.iter(|| {
                let reg = rust_compile(black_box(pat), ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    // Tier 1: real-world scenarios
    bench_scanner_highlighting,
    bench_text_scanning,
    bench_single_pattern,
    bench_compilation,
    // Tier 2: regression coverage
    bench_regression_compile,
    bench_regression_literal,
    bench_regression_quantifiers,
    bench_regression_alternation,
    bench_regression_backreferences,
    bench_regression_lookaround,
    bench_regression_unicode,
    bench_regression_case_insensitive,
    bench_regression_named_captures,
    bench_regression_large_text,
    bench_regression_regset,
    bench_regression_match_at_position,
    bench_regression_scanner,
    bench_regression_scanner_textmate,
    bench_regression_idiomatic_api,
);
criterion_main!(benches);
