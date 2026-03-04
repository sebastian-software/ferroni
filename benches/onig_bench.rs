// Criterion benchmark suite: Ferroni (Rust) vs Oniguruma (C)
//
// Run: cargo bench --features ffi
// Specific group: cargo bench --features ffi -- compile
// HTML report: target/criterion/report/index.html

mod scanner_css_workload;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use scanner_css_workload::{CSS_INPUT, CSS_PATTERNS};
use std::os::raw::c_uint;

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi;
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

fn c_compile(pattern: &[u8], option: c_uint) -> ffi::CRegex {
    ffi::CRegex::new(pattern, option).expect("C compile failed")
}

// Verify both engines agree on match position (debug only)
fn assert_same_result(rust_pos: i32, c_pos: i32, label: &str) {
    debug_assert_eq!(
        rust_pos >= 0,
        c_pos >= 0,
        "{label}: match/mismatch disagree (rust={rust_pos}, c={c_pos})"
    );
}

// ---------------------------------------------------------------------------
// 1. compile -- measure compilation time
// ---------------------------------------------------------------------------

fn bench_compile(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("compile");
    for (name, pat) in patterns {
        group.bench_with_input(BenchmarkId::new("rust", name), pat, |b, pat| {
            b.iter(|| {
                let reg = rust_compile(black_box(pat), ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
        group.bench_with_input(BenchmarkId::new("c", name), pat, |b, pat| {
            b.iter(|| {
                let reg = c_compile(black_box(pat), ffi::ONIG_OPTION_NONE);
                black_box(&reg);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 2. literal_match -- BMH fast-path
// ---------------------------------------------------------------------------

fn bench_literal_match(c: &mut Criterion) {
    let text = b"The quick brown fox jumps over the lazy dog near the riverbank";
    let cases: &[(&str, &[u8])] = &[
        ("exact", b"lazy dog"),
        ("anchored_start", b"^The quick"),
        ("anchored_end", b"riverbank$"),
        ("word_boundary", b"\\bfox\\b"),
    ];

    let mut group = c.benchmark_group("literal_match");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        // Verify agreement
        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 3. quantifiers
// ---------------------------------------------------------------------------

fn bench_quantifiers(c: &mut Criterion) {
    let text = b"aaaaabbbbbccccc12345";
    let cases: &[(&str, &[u8])] = &[
        ("greedy", b"a+b+c+"),
        ("lazy", b"a+?b+?c+?"),
        ("possessive", b"a++b++"),
        ("nested", b"(a+b+)+"),
    ];

    let mut group = c.benchmark_group("quantifiers");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 4. alternation
// ---------------------------------------------------------------------------

fn bench_alternation(c: &mut Criterion) {
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

    let mut group = c.benchmark_group("alternation");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 5. backreferences
// ---------------------------------------------------------------------------

fn bench_backreferences(c: &mut Criterion) {
    let text = b"the the quick brown fox fox jumped over";
    let cases: &[(&str, &[u8])] = &[
        ("simple", b"(\\w+) \\1"),
        ("nested", b"((\\w+) \\2)"),
        ("named", b"(?<word>\\w+) \\k<word>"),
    ];

    let mut group = c.benchmark_group("backreferences");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 6. lookaround
// ---------------------------------------------------------------------------

fn bench_lookaround(c: &mut Criterion) {
    let text = b"price: $42.99 and cost: $10.00 for item";
    let cases: &[(&str, &[u8])] = &[
        ("pos_lookahead", b"\\$\\d+(?=\\.)"),
        ("neg_lookahead", b"\\$\\d+(?!\\.)"),
        ("pos_lookbehind", b"(?<=\\$)\\d+"),
        ("neg_lookbehind", b"(?<!\\$)\\d+"),
        ("combined", b"(?<=\\$)\\d+(?=\\.)"),
    ];

    let mut group = c.benchmark_group("lookaround");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 7. unicode_properties
// ---------------------------------------------------------------------------

fn bench_unicode_properties(c: &mut Criterion) {
    // Mixed-script input: Latin, Greek, Cyrillic, CJK
    let text = "Hello Κόσμε Привет 世界 café résumé naïve".as_bytes();
    let cases: &[(&str, &[u8])] = &[
        ("upper", b"\\p{Lu}+"),
        ("letter", b"\\p{Letter}+"),
        ("greek", b"\\p{Greek}+"),
        ("cyrillic", b"\\p{Cyrillic}+"),
    ];

    let mut group = c.benchmark_group("unicode_properties");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

        let (r_pos, _) = rust_search(&r_reg, text, None);
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), text, |b, text| {
            b.iter(|| {
                let (pos, _) = rust_search(&r_reg, black_box(text), None);
                black_box(pos);
            });
        });
        group.bench_with_input(BenchmarkId::new("c", name), text, |b, text| {
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 8. case_insensitive
// ---------------------------------------------------------------------------

fn bench_case_insensitive(c: &mut Criterion) {
    let text = b"The Quick BROWN Fox Jumps OVER the Lazy DOG";
    let cases: &[(&str, &[u8])] = &[
        ("word", b"quick"),
        ("phrase", b"brown fox"),
        ("alternation", b"quick|lazy|dog"),
    ];

    let mut group = c.benchmark_group("case_insensitive");
    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_IGNORECASE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_IGNORECASE);

        let (r_pos, _) = onig_search(
            &r_reg,
            text,
            text.len(),
            0,
            text.len(),
            None,
            ONIG_OPTION_NONE,
        );
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 9. named_captures -- extract date fields
// ---------------------------------------------------------------------------

fn bench_named_captures(c: &mut Criterion) {
    let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";
    let pat = b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
    let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

    let mut group = c.benchmark_group("named_captures");

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

    group.bench_function("c", |b| {
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
    group.finish();
}

// ---------------------------------------------------------------------------
// 10. large_text -- realistic log scanning
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

fn bench_large_text(c: &mut Criterion) {
    let text_10k = make_log_text(100); // ~10KB
    let text_50k = make_log_text(500); // ~50KB

    let cases: &[(&str, &[u8])] = &[
        ("literal_INFO", b"INFO"),
        ("timestamp", b"\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}"),
        ("field_extract", b"duration=(\\d+)ms"),
        ("no_match", b"CRITICAL_ERROR"),
    ];

    let mut group = c.benchmark_group("large_text");

    for (name, pat) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

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
        group.bench_with_input(BenchmarkId::new("c", &label_10k), &text_10k, |b, text| {
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
        group.bench_with_input(BenchmarkId::new("c", &label_50k), &text_50k, |b, text| {
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
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 11. regset -- multi-pattern matching
// ---------------------------------------------------------------------------

fn bench_regset(c: &mut Criterion) {
    let text = b"Error 404: page not found at /api/users/42 on 2025-06-15";

    let patterns: &[&[u8]] = &[
        b"Error \\d+",
        b"/api/\\w+/\\d+",
        b"\\d{4}-\\d{2}-\\d{2}",
        b"not found",
        b"\\bpage\\b",
    ];

    // Rust regset
    let rust_regs: Vec<Box<ferroni::regint::RegexType>> = patterns
        .iter()
        .map(|p| Box::new(rust_compile(p, ONIG_OPTION_NONE)))
        .collect();
    let (rust_set, rc) = onig_regset_new(rust_regs);
    assert!(rc == 0, "Rust regset_new failed: {rc}");
    let mut rust_set = rust_set.unwrap();

    // C regset -- compile individually, then hand raw pointers to regset
    let c_regs_owned: Vec<ffi::CRegex> = patterns
        .iter()
        .map(|p| c_compile(p, ffi::ONIG_OPTION_NONE))
        .collect();
    let c_raw_ptrs: Vec<ffi::OnigRegex> = c_regs_owned.iter().map(|r| r.raw()).collect();
    // C regset takes ownership of the regex objects, so we must NOT free them.
    // Leak the CRegex wrappers to prevent double-free.
    for r in c_regs_owned {
        std::mem::forget(r);
    }
    let mut c_set = ffi::CRegSet::new(&c_raw_ptrs).expect("C regset_new failed");

    let mut group = c.benchmark_group("regset");

    // Position-lead
    group.bench_function("rust/position_lead", |b| {
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

    group.bench_function("c/position_lead", |b| {
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

    // Regex-lead
    group.bench_function("rust/regex_lead", |b| {
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

    group.bench_function("c/regex_lead", |b| {
        b.iter(|| {
            let (idx, pos) = c_set.search(
                black_box(text),
                0,
                text.len(),
                ffi::ONIG_REGSET_REGEX_LEAD,
                ffi::ONIG_OPTION_NONE,
            );
            black_box((idx, pos));
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 12. match_at_position -- onig_match at a known offset
// ---------------------------------------------------------------------------

fn bench_match_at_position(c: &mut Criterion) {
    let text = b"xxxx1234abcd";
    let pat = b"\\d+";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
    let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

    // Verify: match at offset 4
    let (r_len, _) = onig_match(&r_reg, text, text.len(), 4, None, ONIG_OPTION_NONE);
    let c_len = c_reg.match_at(text, 4, None, ffi::ONIG_OPTION_NONE);
    assert!(r_len == 4, "Rust match_at expected 4, got {r_len}");
    assert!(c_len == 4, "C match_at expected 4, got {c_len}");

    let mut group = c.benchmark_group("match_at_position");

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

    group.bench_function("c", |b| {
        b.iter(|| {
            let len = c_reg.match_at(black_box(text), 4, None, ffi::ONIG_OPTION_NONE);
            black_box(len);
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// 13. scanner -- Scanner API overhead vs raw RegSet/onig_search
// ---------------------------------------------------------------------------

/// Same patterns as bench_regset for direct comparison.
const SCANNER_PATTERNS: &[&str] = &[
    "Error \\d+",
    "/api/\\w+/\\d+",
    "\\d{4}-\\d{2}-\\d{2}",
    "not found",
    "\\bpage\\b",
];

const SCANNER_PATTERNS_BYTES: &[&[u8]] = &[
    b"Error \\d+",
    b"/api/\\w+/\\d+",
    b"\\d{4}-\\d{2}-\\d{2}",
    b"not found",
    b"\\bpage\\b",
];

const SCANNER_TEXT_SHORT: &[u8] = b"Error 404: page not found at /api/users/42 on 2025-06-15";

fn make_long_text() -> Vec<u8> {
    // ~2KB — above the 1000-byte threshold for per-regex path
    let base = b"Error 404: page not found at /api/users/42 on 2025-06-15. ";
    let mut text = Vec::with_capacity(base.len() * 40);
    for _ in 0..40 {
        text.extend_from_slice(base);
    }
    text
}

fn bench_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanner");

    // -- short_string: Ferroni Scanner (RegSet fast-path) --
    {
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();
        let text = std::str::from_utf8(SCANNER_TEXT_SHORT).unwrap();

        group.bench_function("short_string", |b| {
            b.iter(|| {
                let m = scanner.find_next_match(black_box(text), 0, ScannerFindOptions::NONE);
                black_box(m);
            });
        });
    }

    // -- short_string_c: vscode-oniguruma C scanner (RegSet fast-path) --
    {
        let c_scanner =
            ffi::CScanner::new(SCANNER_PATTERNS_BYTES).expect("C scanner create failed");

        group.bench_function("short_string_c", |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(SCANNER_TEXT_SHORT), 0, 0);
                black_box(m);
            });
        });
    }

    // -- short_string_c_raw: raw C RegSet (no scanner layer, pure engine) --
    {
        let c_regs_owned: Vec<ffi::CRegex> = SCANNER_PATTERNS_BYTES
            .iter()
            .map(|p| c_compile(p, ffi::ONIG_OPTION_NONE))
            .collect();
        let c_raw_ptrs: Vec<ffi::OnigRegex> = c_regs_owned.iter().map(|r| r.raw()).collect();
        for r in c_regs_owned {
            std::mem::forget(r);
        }
        let mut c_set = ffi::CRegSet::new(&c_raw_ptrs).expect("C regset_new failed");

        group.bench_function("short_string_c_raw", |b| {
            b.iter(|| {
                let (idx, pos) = c_set.search(
                    black_box(SCANNER_TEXT_SHORT),
                    0,
                    SCANNER_TEXT_SHORT.len(),
                    ffi::ONIG_REGSET_POSITION_LEAD,
                    ffi::ONIG_OPTION_NONE,
                );
                black_box((idx, pos));
            });
        });
    }

    // -- long_string_cold: Ferroni per-regex path, no caching --
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

    // -- long_string_cold_c: vscode-oniguruma C scanner, no caching --
    //    Use incrementing strCacheId so the cache never hits.
    {
        let long = make_long_text();
        let c_scanner =
            ffi::CScanner::new(SCANNER_PATTERNS_BYTES).expect("C scanner create failed");
        let mut cache_id = 100i32;

        group.bench_function("long_string_cold_c", |b| {
            b.iter(|| {
                cache_id = cache_id.wrapping_add(1);
                let m = c_scanner.find_next_match(black_box(&long), cache_id, 0);
                black_box(m);
            });
        });
    }

    // -- long_string_cold_c_raw: raw C per-regex search (no scanner, no caching) --
    //    Mirrors what the scanner does internally: search each regex, pick earliest.
    {
        let long = make_long_text();
        let c_regs: Vec<ffi::CRegex> = SCANNER_PATTERNS_BYTES
            .iter()
            .map(|p| c_compile(p, ffi::ONIG_OPTION_NONE))
            .collect();

        group.bench_function("long_string_cold_c_raw", |b| {
            let mut region = ffi::CRegion::new();
            b.iter(|| {
                let text = black_box(long.as_slice());
                let mut best_pos: i32 = -1;
                let mut best_idx: i32 = -1;
                for (i, reg) in c_regs.iter().enumerate() {
                    region.clear();
                    let pos = reg.search(
                        text,
                        0,
                        text.len(),
                        Some(&mut region),
                        ffi::ONIG_OPTION_NONE,
                    );
                    if pos >= 0 && (best_pos < 0 || pos < best_pos) {
                        best_pos = pos;
                        best_idx = i as i32;
                        if pos == 0 {
                            break;
                        }
                    }
                }
                black_box((best_idx, best_pos));
            });
        });
    }

    // -- long_string_warm: Ferroni per-regex path, same str_id → cache hits --
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

    // -- long_string_warm_c: vscode-oniguruma C scanner, warm cache (same strCacheId) --
    {
        let long = make_long_text();
        let c_scanner =
            ffi::CScanner::new(SCANNER_PATTERNS_BYTES).expect("C scanner create failed");

        // Prime the cache
        c_scanner.find_next_match(&long, 1, 0);

        group.bench_function("long_string_warm_c", |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(&long), 1, 0);
                black_box(m);
            });
        });
    }

    // -- utf16: OnigString creation + find_next_match_utf16 (no C equivalent) --
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

    // -- css workload: single representative tokenize benchmark --
    {
        let patterns = valid_css_patterns();
        let pattern_count = patterns.len();
        let patterns_bytes = valid_css_patterns_bytes(&patterns);
        let patterns_byte_refs: Vec<&[u8]> = patterns_bytes.iter().map(|v| v.as_slice()).collect();

        let content = CSS_INPUT;
        let content_bytes = content.as_bytes();
        let onig_str = OnigString::new(content);

        // tokenize full CSS input
        {
            let mut scanner = Scanner::new(&patterns).unwrap();
            let c_scanner =
                ffi::CScanner::new(&patterns_byte_refs).expect("C scanner create failed");

            let input_len_utf16 = content.encode_utf16().count();
            let label = format!("css_{pattern_count}_patterns_tokenize_rust");
            group.bench_function(&label, |b| {
                b.iter(|| {
                    let mut pos = 0usize;
                    let mut count = 0u32;
                    while pos < input_len_utf16 {
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

            let input_len_bytes = content_bytes.len();
            let label = format!("css_{pattern_count}_patterns_tokenize_c");
            group.bench_function(&label, |b| {
                b.iter(|| {
                    let mut pos = 0usize;
                    let mut count = 0u32;
                    while pos < input_len_bytes {
                        if let Some((_idx, captures)) =
                            c_scanner.find_next_match(black_box(content_bytes), 0, pos)
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
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// 14. scanner_textmate -- TextMate-realistic Scanner workload (63 patterns)
// ---------------------------------------------------------------------------

// 65 patterns extracted from TypeScript grammar "expression" group (shiki-clean).
// These are the actual regexes compiled into a single Scanner during TS tokenization.
// See https://github.com/sebastian-software/ferroni/issues/6
const TS_EXPRESSION_PATTERNS: &[&str] = &[
    "'",
    "\"",
    "([$_[:alpha:]][$_[:alnum:]]*)?(`)",
    "(?<!\\+\\+|--|})(?<=[!(+,:=?\\[]|^return|[^$._[:alnum:]]return|^case|[^$._[:alnum:]]case|=>|&&|\\|\\||\\*/)\\s*(/)(?![*/])(?=(?:[^()/\\[\\\\]|\\\\.|\\[([^]\\\\]|\\\\.)+]|\\(([^)\\\\]|\\\\.)+\\))+/([dgimsuvy]+|(?![*/])|(?=/\\*))(?!\\s*[$0-9A-Z_a-z]))",
    "((?<![]$)_[:alnum:]]|\\+\\+|--|}|\\*/)|((?<=^return|[^$._[:alnum:]]return|^case|[^$._[:alnum:]]case))\\s*)/(?![*/])(?=(?:[^/\\[\\\\]|\\\\.|\\[([^]\\\\]|\\\\.)*])+/([dgimsuvy]+|(?![*/])|(?=/\\*))(?!\\s*[$0-9A-Z_a-z]))",
    "/\\*\\*(?!/)",
    "(^[\\t ]+)?((//)(?:\\s*((@)internal)(?=\\s|$))?)",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(?:(async)\\s+)?(function)\\b(?:\\s*(\\*))?(?:(?:\\s+|(?<=\\*))([$_[:alpha:]][$_[:alnum:]]*))?\\s*",
    "[$_[:alpha:]][$_[:alnum:]]*",
    "\\*",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(?:(abstract)\\s+)?(class)\\b(?=\\s+|[<{]|/[*/])",
    "(?:(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))\\b(async)\\s+)?([$_[:alpha:]][$_[:alnum:]]*)\\s*(?==>)",
    "(?:(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))\\b(async))?((?<![]!)}])\\s*(?=((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)?\\(\\s*(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*((\\)\\s*:)|((\\.\\.\\.\\s*)?[$_[:alpha:]][$_[:alnum:]]*\\s*:)))|((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)?\\(\\s*(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*(([$_[:alpha:]]|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*])|(\\.\\.\\.\\s*[$_[:alpha:]]))([^\"'()`]|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|('([^'\\\\]|\\\\.)*')|(\"([^\"\\\\]|\\\\.)*\")|(`([^\\\\`]|\\\\.)*`))*)?\\)(\\s*:\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+)?\\s*=>)))",
    "=>",
    "(?<=[(,=])\\s*(async)?(?=\\s*((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*))?\\(\\s*((([\\[{]\\s*)?)$|((\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})\\s*((:\\s*\\{?)$|((\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+\\s*)?=\\s*)))|((\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*])\\s*((:\\s*\\[?)$|((\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+\\s*)?=\\s*)))))",
    "(?<=[(,=]|=>|^return|[^$._[:alnum:]]return|^throw|[^$._[:alnum:]]throw|^yield|[^$._[:alnum:]]yield|^await|[^$._[:alnum:]]await|^default|[^$._[:alnum:]]default|[\\&(*,:=>?^|]|[^$_[:alnum:]](?:\\+\\+|--)|[^+]\\+|[^-]-)\\s*(async)?(?=\\s*((((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*))?\\()|(<)|((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)))\\s*$)",
    "(?<=\\)|^)\\s*(:)(?=\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+\\s*=>)",
    "\\s*(<)\\s*(const)\\s*(>)",
    "(?<!\\+\\+|--)(?<=^return|[^$._[:alnum:]]return|^throw|[^$._[:alnum:]]throw|^yield|[^$._[:alnum:]]yield|^await|[^$._[:alnum:]]await|^default|[^$._[:alnum:]]default|[\\&(*,:=>?^|]|[^$_[:alnum:]](?:\\+\\+|--)|[^+]\\+|[^-]-)\\s*(<)(?!<?=)(?!\\s*$)",
    "(?<=^)\\s*(<)(?=[$_[:alpha:]][$_[:alnum:]]*\\s*>)",
    "(?!\\?\\.\\s*\\D)(\\?)(?!\\?)",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(new)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(instanceof)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(readonly)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))\\s*",
    "\\{",
    "(?=\\[)",
    "(?=[\"'`])",
    "(?=\\b((?<!\\$)0[Xx]\\h[_\\h]*(n)?\\b(?!\\$))|\\b((?<!\\$)0[Bb][01][01_]*(n)?\\b(?!\\$))|\\b((?<!\\$)0[Oo]?[0-7][0-7_]*(n)?\\b(?!\\$))|((?<!\\$)(?:\\b[0-9][0-9_]*(\\.)[0-9][0-9_]*[Ee][-+]?[0-9][0-9_]*(n)?\\b|\\b[0-9][0-9_]*(\\.)[Ee][-+]?[0-9][0-9_]*(n)?\\b|\\B(\\.)[0-9][0-9_]*[Ee][-+]?[0-9][0-9_]*(n)?\\b|\\b[0-9][0-9_]*[Ee][-+]?[0-9][0-9_]*(n)?\\b|\\b[0-9][0-9_]*(\\.)[0-9][0-9_]*(n)?\\b|\\b[0-9][0-9_]*(\\.)(n)?\\B|\\B(\\.)[0-9][0-9_]*(n)?\\b|\\b[0-9][0-9_]*(n)?\\b(?!\\.))(?!\\$)))",
    "(?<=[]\"'`])(?=\\s*[(<])",
    "(?![$_[:alpha:]])(\\d+)\\s*(?=(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*:)",
    "([$_[:alpha:]][$_[:alnum:]]*)\\s*(?=(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*:(\\s*/\\*([^*]|(\\*[^/]))*\\*/)*\\s*(((async\\s+)?((function\\s*[(*<])|(function\\s+)|([$_[:alpha:]][$_[:alnum:]]*\\s*=>)))|((async\\s*)?(((<\\s*)$|((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)?\\(\\s*((([\\[{]\\s*)?)$|((\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})\\s*((:\\s*\\{?)$|((\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+\\s*)?=\\s*)))|((\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*])\\s*((:\\s*\\[?)$|((\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+\\s*)?=\\s*))))))|((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)?\\(\\s*(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*((\\)\\s*:)|((\\.\\.\\.\\s*)?[$_[:alpha:]][$_[:alnum:]]*\\s*:)))|((<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<]|<\\s*(((const\\s+)?[$_[:alpha:]])|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*]))([^<=>]|=[^<])*>)*>)*>\\s*)?\\(\\s*(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*(([$_[:alpha:]]|(\\{([^{}]|(\\{([^{}]|\\{[^{}]*})*}))*})|(\\[([^]\\[]|(\\[([^]\\[]|\\[[^]\\[]*])*]))*])|(\\.\\.\\.\\s*[$_[:alpha:]]))([^\"'()`]|(\\(([^()]|(\\(([^()]|\\([^()]*\\))*\\)))*\\))|('([^'\\\\]|\\\\.)*')|(\"([^\"\\\\]|\\\\.)*\")|(`([^\\\\`]|\\\\.)*`))*)?\\)(\\s*:\\s*([^()<>{}]|<([^<>]|<([^<>]|<[^<>]+>)+>)+>|\\([^()]+\\)|\\{[^{}]+})+)?\\s*=>)))))",
    "[$_[:alpha:]][$_[:alnum:]]*\\s*(?=(/\\*([^*]|(\\*[^/]))*\\*/\\s*)*:)",
    "\\.\\.\\.",
    "([$_[:alpha:]][$_[:alnum:]]*)\\s*(?=[,}]|$|//|/\\*)",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(as)\\s+(const)(?=\\s*([,}]|$))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(?:(as)|(satisfies))\\s+",
    "(?=[$_[:alpha:]][$_[:alnum:]]*\\s*=)",
    ":",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(await)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(yield)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))(?=\\s*/\\*([^*]|(\\*[^/]))*\\*/\\s*\\*)",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(yield)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))(?:\\s*(\\*))?",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))delete(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))in(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))(?!\\()",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))of(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))(?!\\()",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(instanceof)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(new)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(typeof)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(void)(?![$_[:alnum:]])(?:(?=\\.\\.\\.)|(?!\\.))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(as)\\s+(const)(?=\\s*($|[]),:;}]))",
    "(?<![$_[:alnum:]])(?:(?<=\\.\\.\\.)|(?<!\\.))(?:(as)|(satisfies))\\s+",
    "\\.\\.\\.",
    "(?:\\*|(?<!\\()/|[-%+])=",
    "(?:[\\&^]|<<|>>>??|\\|)=",
    "<<|>>>?",
    "[!=]==?",
    "<=|>=|<>|[<>]",
    "(?<=[$_[:alnum:]])(!)\\s*(?:(/=)|(/)(?![*/]))",
    "!|&&|\\|\\||\\?\\?",
    "[\\&^|~]",
    "=",
    "--",
    "\\+\\+",
];

/// Filter TS_EXPRESSION_PATTERNS to only those that compile successfully.
fn valid_ts_patterns() -> Vec<&'static str> {
    TS_EXPRESSION_PATTERNS
        .iter()
        .copied()
        .filter(|p| Scanner::new(&[*p]).is_ok())
        .collect()
}

/// Convert valid patterns to byte slices for C scanner.
fn valid_ts_patterns_bytes(patterns: &[&str]) -> Vec<Vec<u8>> {
    patterns.iter().map(|p| p.as_bytes().to_vec()).collect()
}

fn bench_scanner_textmate(c: &mut Criterion) {
    let patterns = valid_ts_patterns();
    let pattern_count = patterns.len();
    let patterns_bytes = valid_ts_patterns_bytes(&patterns);
    let patterns_byte_refs: Vec<&[u8]> = patterns_bytes.iter().map(|v| v.as_slice()).collect();
    let mut group = c.benchmark_group("scanner-textmate");

    let content = "const result = await fetchUsers({ limit: 100, offset: 0 }); // API call";
    let content_bytes = content.as_bytes();
    let onig_str = OnigString::new(content);

    // -- compile: Rust Scanner::new --
    {
        let label = format!("compile_{pattern_count}_rust");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // -- compile: C CScanner::new --
    {
        let label = format!("compile_{pattern_count}_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner =
                    ffi::CScanner::new(black_box(&patterns_byte_refs)).expect("C scanner failed");
                black_box(scanner);
            });
        });
    }

    // -- match short: Rust --
    {
        let mut scanner = Scanner::new(&patterns).unwrap();
        let label = format!("{pattern_count}_patterns_short_rust");
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

    // -- match short: C --
    {
        let c_scanner = ffi::CScanner::new(&patterns_byte_refs).expect("C scanner create failed");
        let label = format!("{pattern_count}_patterns_short_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(content_bytes), 0, 0);
                black_box(m);
            });
        });
    }

    // verify both engines agree on first match
    {
        let mut scanner = Scanner::new(&patterns).unwrap();
        let c_scanner = ffi::CScanner::new(&patterns_byte_refs).expect("C scanner create failed");
        let rust_m = scanner.find_next_match(
            std::str::from_utf8(content_bytes).unwrap(),
            0,
            ScannerFindOptions::NONE,
        );
        let c_m = c_scanner.find_next_match(content_bytes, 0, 0);
        debug_assert_eq!(
            rust_m.map(|m| m.index as usize),
            c_m.map(|m| m.0),
            "Rust/C disagree on first match pattern index"
        );
    }

    // -- tokenize full line: Rust --
    {
        let mut scanner = Scanner::new(&patterns).unwrap();
        let line_len = content.encode_utf16().count();
        let label = format!("{pattern_count}_patterns_tokenize_rust");
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

    // -- tokenize full line: C --
    {
        let c_scanner = ffi::CScanner::new(&patterns_byte_refs).expect("C scanner create failed");
        let label = format!("{pattern_count}_patterns_tokenize_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                let content_len = content_bytes.len();
                while pos < content_len {
                    if let Some((_idx, captures)) =
                        c_scanner.find_next_match(black_box(content_bytes), 0, pos)
                    {
                        let end = captures[0].1 as usize; // capture 0 end
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

// ---------------------------------------------------------------------------
// CSS scanner helpers used by the integrated scanner benchmarks
// ---------------------------------------------------------------------------

fn valid_css_patterns() -> Vec<&'static str> {
    CSS_PATTERNS
        .iter()
        .copied()
        .filter(|p| Scanner::new(&[*p]).is_ok())
        .collect()
}

fn valid_css_patterns_bytes(patterns: &[&str]) -> Vec<Vec<u8>> {
    patterns.iter().map(|p| p.as_bytes().to_vec()).collect()
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_compile,
    bench_literal_match,
    bench_quantifiers,
    bench_alternation,
    bench_backreferences,
    bench_lookaround,
    bench_unicode_properties,
    bench_case_insensitive,
    bench_named_captures,
    bench_large_text,
    bench_regset,
    bench_match_at_position,
    bench_scanner,
    bench_scanner_textmate,
);
criterion_main!(benches);
