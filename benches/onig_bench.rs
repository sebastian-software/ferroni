// Criterion benchmark suite: Ferroni (Rust) vs Oniguruma (C) vs regex crate
//
// Run: cargo bench --features ffi
// Specific group: cargo bench --features ffi -- compile
// HTML report: target/criterion/report/index.html

mod grammar_loader;
mod scanner_css_workload;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use regex::bytes::{Regex, RegexBuilder};
use scanner_css_workload::CSS_INPUT;
use std::os::raw::c_uint;
use std::time::Duration;

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi;
use ferroni::oniguruma::{OnigOptionType, OnigRegion, ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{onig_match, onig_region_new, onig_search};
use ferroni::regset::{onig_regset_new, onig_regset_search, OnigRegSetLead};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::scanner::{OnigString, Scanner, ScannerFindOptions};

fn is_smoke_benchmark() -> bool {
    matches!(
        std::env::var("FERRONI_BENCH_SMOKE")
            .unwrap_or_else(|_| String::new())
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

// ---------------------------------------------------------------------------
// Smoke benchmark (5-10 high-signal kernels, fast iteration)
// ---------------------------------------------------------------------------

fn bench_smoke_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("smoke_compare");
    group.sample_size(40);
    group.warm_up_time(Duration::from_millis(200));
    group.measurement_time(Duration::from_secs(2));

    // --------------------------------------------------------------------
    // Compile throughput (small set)
    // --------------------------------------------------------------------
    let compile_cases: &[(&str, &[u8])] = &[
        ("literal", b"hello world"),
        (
            "named_capture",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
        ),
    ];

    for (name, pattern) in compile_cases {
        group.bench_function(format!("compile/{name}/rust"), |b| {
            b.iter(|| {
                let reg = rust_compile(black_box(pattern), ONIG_OPTION_NONE);
                black_box(reg);
            })
        });
        group.bench_function(format!("compile/{name}/c"), |b| {
            b.iter(|| {
                let reg = c_compile(black_box(pattern), ffi::ONIG_OPTION_NONE);
                black_box(reg);
            })
        });
    }

    // --------------------------------------------------------------------
    // Representative execution kernels
    // --------------------------------------------------------------------
    let text_common = b"The quick brown fox jumps over the lazy dog near 2025-06-15";
    let lit = b"lazy dog";
    let alt = b"wolf|wolverine";
    let backref = b"(\\w+) \\1";
    let greek = b"\\p{Greek}+";
    let unicode_text = "Hello Κόσμε Привет 世界".as_bytes();

    let rust_lit = rust_compile(lit, ONIG_OPTION_NONE);
    let c_lit = c_compile(lit, ffi::ONIG_OPTION_NONE);
    let rust_alt = rust_compile(alt, ONIG_OPTION_NONE);
    let c_alt = c_compile(alt, ffi::ONIG_OPTION_NONE);
    let rust_backref = rust_compile(backref, ONIG_OPTION_NONE);
    let c_backref = c_compile(backref, ffi::ONIG_OPTION_NONE);
    let rust_greek = rust_compile(greek, ONIG_OPTION_NONE);
    let c_greek = c_compile(greek, ffi::ONIG_OPTION_NONE);

    let (rust_lit_match, _) = rust_search(&rust_lit, text_common, None);
    let c_lit_match = c_lit.search(
        text_common,
        0,
        text_common.len(),
        None,
        ffi::ONIG_OPTION_NONE,
    );
    assert_same_result(rust_lit_match, c_lit_match, "literal");

    let (rust_alt_match, _) = rust_search(&rust_alt, text_common, None);
    let c_alt_match = c_alt.search(
        text_common,
        0,
        text_common.len(),
        None,
        ffi::ONIG_OPTION_NONE,
    );
    assert_same_result(rust_alt_match, c_alt_match, "alternation");

    let utf8_text = b"the the quick brown fox";
    let (rust_backref_match, _) = rust_search(&rust_backref, utf8_text, None);
    let c_backref_match =
        c_backref.search(utf8_text, 0, utf8_text.len(), None, ffi::ONIG_OPTION_NONE);
    assert_same_result(rust_backref_match, c_backref_match, "backref");

    let (rust_greek_match, _) = rust_search(&rust_greek, unicode_text, None);
    let c_greek_match = c_greek.search(
        unicode_text,
        0,
        unicode_text.len(),
        None,
        ffi::ONIG_OPTION_NONE,
    );
    assert_same_result(rust_greek_match, c_greek_match, "unicode_greek");

    group.bench_function("search/literal/rust", |b| {
        b.iter(|| {
            let (pos, _) = rust_search(&rust_lit, black_box(text_common), None);
            black_box(pos);
        })
    });
    let mut lit_region = ffi::CRegion::new();
    group.bench_function("search/literal/c", |b| {
        b.iter(|| {
            lit_region.clear();
            let pos = c_lit.search(
                black_box(text_common),
                0,
                text_common.len(),
                Some(&mut lit_region),
                ffi::ONIG_OPTION_NONE,
            );
            black_box(pos);
        })
    });

    group.bench_function("search/alternation/rust", |b| {
        b.iter(|| {
            let (pos, _) = rust_search(&rust_alt, black_box(text_common), None);
            black_box(pos);
        })
    });
    let mut alt_region = ffi::CRegion::new();
    group.bench_function("search/alternation/c", |b| {
        b.iter(|| {
            alt_region.clear();
            let pos = c_alt.search(
                black_box(text_common),
                0,
                text_common.len(),
                Some(&mut alt_region),
                ffi::ONIG_OPTION_NONE,
            );
            black_box(pos);
        })
    });

    group.bench_function("search/backref/rust", |b| {
        b.iter(|| {
            let (pos, _) = rust_search(&rust_backref, black_box(utf8_text), None);
            black_box(pos);
        })
    });
    let mut backref_region = ffi::CRegion::new();
    group.bench_function("search/backref/c", |b| {
        b.iter(|| {
            backref_region.clear();
            let pos = c_backref.search(
                black_box(utf8_text),
                0,
                utf8_text.len(),
                Some(&mut backref_region),
                ffi::ONIG_OPTION_NONE,
            );
            black_box(pos);
        })
    });

    group.bench_function("search/unicode_greek/rust", |b| {
        b.iter(|| {
            let (pos, _) = rust_search(&rust_greek, black_box(unicode_text), None);
            black_box(pos);
        })
    });
    let mut greek_region = ffi::CRegion::new();
    group.bench_function("search/unicode_greek/c", |b| {
        b.iter(|| {
            greek_region.clear();
            let pos = c_greek.search(
                black_box(unicode_text),
                0,
                unicode_text.len(),
                Some(&mut greek_region),
                ffi::ONIG_OPTION_NONE,
            );
            black_box(pos);
        })
    });

    group.bench_function("match_at_position/rust", |b| {
        b.iter(|| {
            let (len, _) = onig_match(
                &rust_backref,
                text_common,
                text_common.len(),
                4,
                None,
                ONIG_OPTION_NONE,
            );
            black_box(len);
        })
    });
    group.bench_function("match_at_position/c", |b| {
        b.iter(|| {
            let len = c_backref.match_at(text_common, 4, None, ffi::ONIG_OPTION_NONE);
            black_box(len);
        })
    });

    // --------------------------------------------------------------------
    // Scanner + RegSet kernel mix
    // --------------------------------------------------------------------
    let regset_patterns: &[&[u8]] = &[
        b"Error \\d+",
        b"/api/\\w+/\\d+",
        b"\\d{4}-\\d{2}-\\d{2}",
        b"not found",
        b"\\bpage\\b",
    ];
    let regset_text = b"Error 404: page not found at /api/users/42 on 2025-06-15";
    let rust_regs: Vec<Box<ferroni::regint::RegexType>> = regset_patterns
        .iter()
        .map(|pat| Box::new(rust_compile(pat, ONIG_OPTION_NONE)))
        .collect();
    let (rust_set, rc) = onig_regset_new(rust_regs);
    assert!(rc == 0, "Rust regset_new failed: {rc}");
    let mut rust_set = rust_set.unwrap();

    let c_regs_owned: Vec<ffi::CRegex> = regset_patterns
        .iter()
        .map(|pat| c_compile(pat, ffi::ONIG_OPTION_NONE))
        .collect();
    let c_raw_ptrs: Vec<ffi::OnigRegex> = c_regs_owned.iter().map(|r| r.raw()).collect();
    for r in c_regs_owned {
        std::mem::forget(r);
    }
    let mut c_set = ffi::CRegSet::new(&c_raw_ptrs).expect("C regset_new failed");

    group.bench_function("regset/position_lead/rust", |b| {
        b.iter(|| {
            let (idx, pos) = onig_regset_search(
                &mut rust_set,
                black_box(regset_text),
                regset_text.len(),
                0,
                regset_text.len(),
                OnigRegSetLead::PositionLead,
                ONIG_OPTION_NONE,
            );
            black_box((idx, pos));
        })
    });
    group.bench_function("regset/position_lead/c", |b| {
        b.iter(|| {
            let (idx, pos) = c_set.search(
                black_box(regset_text),
                0,
                regset_text.len(),
                ffi::ONIG_REGSET_POSITION_LEAD,
                ffi::ONIG_OPTION_NONE,
            );
            black_box((idx, pos));
        })
    });

    let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();
    let c_scanner = ffi::CScanner::new(SCANNER_PATTERNS_BYTES).expect("C scanner create failed");
    let scanner_text = std::str::from_utf8(SCANNER_TEXT_SHORT).unwrap();
    group.bench_function("scanner/short_rust", |b| {
        b.iter(|| {
            let m = scanner.find_next_match(black_box(scanner_text), 0, ScannerFindOptions::NONE);
            black_box(m);
        })
    });
    group.bench_function("scanner/short_c", |b| {
        b.iter(|| {
            let m = c_scanner.find_next_match(black_box(SCANNER_TEXT_SHORT), 0, 0);
            black_box(m);
        })
    });
    group.finish();
}

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

fn regex_compile(pattern: &[u8], case_insensitive: bool) -> Regex {
    let pat = std::str::from_utf8(pattern).expect("pattern is not UTF-8");
    RegexBuilder::new(pat)
        .case_insensitive(case_insensitive)
        .unicode(true)
        .build()
        .expect("regex compile failed")
}

// Verify both engines agree on match position (debug only)
fn assert_same_result(rust_pos: i32, c_pos: i32, label: &str) {
    debug_assert_eq!(
        rust_pos >= 0,
        c_pos >= 0,
        "{label}: match/mismatch disagree (rust={rust_pos}, c={c_pos})"
    );
}

// ===========================================================================
// Tier 2: Regression benchmarks (per-feature coverage)
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
// regression: unicode_properties
// ---------------------------------------------------------------------------

fn bench_regression_unicode(c: &mut Criterion) {
    // Mixed-script input: Latin, Greek, Cyrillic, CJK
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
// regression: named_captures -- extract date fields
// ---------------------------------------------------------------------------

fn bench_regression_named_captures(c: &mut Criterion) {
    let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";
    let pat = b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
    let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

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

    let mut group = c.benchmark_group("regression_regset");

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
// regression: match_at_position -- onig_match at a known offset
// ---------------------------------------------------------------------------

fn bench_regression_match_at_position(c: &mut Criterion) {
    let text = b"xxxx1234abcd";
    let pat = b"\\d+";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
    let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);

    // Verify: match at offset 4
    let (r_len, _) = onig_match(&r_reg, text, text.len(), 4, None, ONIG_OPTION_NONE);
    let c_len = c_reg.match_at(text, 4, None, ffi::ONIG_OPTION_NONE);
    assert!(r_len == 4, "Rust match_at expected 4, got {r_len}");
    assert!(c_len == 4, "C match_at expected 4, got {c_len}");

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

    group.bench_function("c", |b| {
        b.iter(|| {
            let len = c_reg.match_at(black_box(text), 4, None, ffi::ONIG_OPTION_NONE);
            black_box(len);
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// regression: scanner -- Scanner API overhead vs raw RegSet/onig_search
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

fn bench_regression_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_scanner");

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
        let css_all = grammar_loader::css_patterns();
        let patterns: Vec<&str> = css_all.iter().map(|s| s.as_str()).collect();
        let pattern_count = patterns.len();
        let patterns_bytes = patterns_to_bytes(&patterns);
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

/// Convert pattern slices to byte vectors for C scanner.
fn patterns_to_bytes(patterns: &[&str]) -> Vec<Vec<u8>> {
    patterns.iter().map(|p| p.as_bytes().to_vec()).collect()
}

fn bench_regression_scanner_textmate(c: &mut Criterion) {
    let ts_all = grammar_loader::typescript_patterns();
    let patterns: Vec<&str> = ts_all.iter().map(|s| s.as_str()).collect();
    let pattern_count = patterns.len();
    let patterns_bytes = patterns_to_bytes(&patterns);
    let patterns_byte_refs: Vec<&[u8]> = patterns_bytes.iter().map(|v| v.as_slice()).collect();
    let mut group = c.benchmark_group("regression_scanner_textmate");

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

// ===========================================================================
// Tier 1: Real-world scenario benchmarks (README-facing, Rust vs C)
// ===========================================================================

// ---------------------------------------------------------------------------
// scanner_highlighting -- the Shiki / VS Code / TextMate workload
// ---------------------------------------------------------------------------

fn bench_scanner_highlighting(c: &mut Criterion) {
    // Load full, unmodified Shiki grammars
    let ts_all = grammar_loader::typescript_patterns();
    let ts_patterns: Vec<&str> = ts_all.iter().map(|s| s.as_str()).collect();
    let ts_count = ts_patterns.len();
    let ts_patterns_bytes = patterns_to_bytes(&ts_patterns);
    let ts_patterns_byte_refs: Vec<&[u8]> =
        ts_patterns_bytes.iter().map(|v| v.as_slice()).collect();

    let css_all = grammar_loader::css_patterns();
    let css_patterns: Vec<&str> = css_all.iter().map(|s| s.as_str()).collect();
    let css_count = css_patterns.len();
    let css_patterns_bytes = patterns_to_bytes(&css_patterns);
    let css_patterns_byte_refs: Vec<&[u8]> =
        css_patterns_bytes.iter().map(|v| v.as_slice()).collect();

    let rust_all = grammar_loader::rust_patterns();
    let rust_patterns: Vec<&str> = rust_all.iter().map(|s| s.as_str()).collect();
    let rust_count = rust_patterns.len();
    let rust_patterns_bytes = patterns_to_bytes(&rust_patterns);
    let rust_patterns_byte_refs: Vec<&[u8]> =
        rust_patterns_bytes.iter().map(|v| v.as_slice()).collect();

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

    // -- TypeScript: compile --
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

    // -- TypeScript: first match --
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

    // -- TypeScript: tokenize line --
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

    // -- CSS: compile --
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

    // -- CSS: tokenize --
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

        let css_byte_len = css_input_bytes.len();
        let label = format!("css_{css_count}_tokenize_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < css_byte_len {
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

    // -- Rust: compile --
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

    // -- Rust: first match --
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

    // -- Rust: tokenize line --
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

        let rust_byte_len = rust_line_bytes.len();
        let label = format!("rust_{rust_count}_tokenize_c");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let mut pos = 0usize;
                let mut count = 0u32;
                while pos < rust_byte_len {
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

    // -- warm cache: scanner with primed cache (steady-state path) --
    {
        let long = make_long_text();
        let long_str = std::str::from_utf8(&long).unwrap();
        let mut scanner = Scanner::new(SCANNER_PATTERNS).unwrap();
        scanner.find_next_match_with_id(long_str, 1, 0, ScannerFindOptions::NONE);

        let c_scanner =
            ffi::CScanner::new(SCANNER_PATTERNS_BYTES).expect("C scanner create failed");
        c_scanner.find_next_match(&long, 1, 0);

        group.bench_function("warm_cache_rust", |b| {
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

        group.bench_function("warm_cache_c", |b| {
            b.iter(|| {
                let m = c_scanner.find_next_match(black_box(&long), 1, 0);
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

    for (name, pat, text) in cases {
        let r_reg = rust_compile(pat, ONIG_OPTION_NONE);
        let c_reg = c_compile(pat, ffi::ONIG_OPTION_NONE);
        let re = regex_compile(pat, false);

        group.bench_with_input(
            BenchmarkId::new("rust", name),
            &text.as_slice(),
            |b, text| {
                b.iter(|| {
                    let (pos, _) = rust_search(&r_reg, black_box(text), None);
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
                    let m = re.find(black_box(text));
                    black_box(m);
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

        let c_regs_owned: Vec<ffi::CRegex> = patterns
            .iter()
            .map(|p| c_compile(p, ffi::ONIG_OPTION_NONE))
            .collect();
        let c_raw_ptrs: Vec<ffi::OnigRegex> = c_regs_owned.iter().map(|r| r.raw()).collect();
        for r in c_regs_owned {
            std::mem::forget(r);
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

// ---------------------------------------------------------------------------
// single_pattern -- one representative per regex feature category
// ---------------------------------------------------------------------------

fn bench_single_pattern(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_pattern");

    // (name, pattern, text, rust_option, c_option, regex_compatible)
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
            false, // regex crate does not support lookaround
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
            false, // regex crate does not support backreferences
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

    for (name, pat, text, r_option, c_option, regex_compat) in cases {
        let r_reg = rust_compile(pat, *r_option);
        let c_reg = c_compile(pat, *c_option);

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

        if *regex_compat {
            let case_insensitive = *r_option == ONIG_OPTION_IGNORECASE;
            let re = regex_compile(pat, case_insensitive);
            group.bench_with_input(BenchmarkId::new("regex", name), &text[..], |b, text| {
                b.iter(|| {
                    let m = re.find(black_box(text));
                    black_box(m);
                });
            });
        }
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// compilation -- representative compile-time spread
// ---------------------------------------------------------------------------

fn bench_compilation(c: &mut Criterion) {
    // (name, pattern, regex_compatible)
    let cases: &[(&str, &[u8], bool)] = &[
        ("literal", b"hello world", true),
        (
            "named_capture",
            b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})",
            true,
        ),
        ("lookbehind", b"(?<=@)\\w+", false), // regex crate does not support lookbehind
    ];

    let mut group = c.benchmark_group("compilation");
    for (name, pat, regex_compat) in cases {
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
        if *regex_compat {
            let pat_str = std::str::from_utf8(pat).unwrap();
            group.bench_with_input(BenchmarkId::new("regex", name), pat, |b, _pat| {
                b.iter(|| {
                    let re = Regex::new(black_box(pat_str)).unwrap();
                    black_box(&re);
                });
            });
        }
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

fn bench_onig_bench(c: &mut Criterion) {
    if is_smoke_benchmark() {
        bench_smoke_compare(c);
        return;
    }

    // Tier 1: real-world scenarios
    bench_scanner_highlighting(c);
    bench_text_scanning(c);
    bench_single_pattern(c);
    bench_compilation(c);
    // Tier 2: regression coverage
    bench_regression_compile(c);
    bench_regression_literal(c);
    bench_regression_quantifiers(c);
    bench_regression_alternation(c);
    bench_regression_backreferences(c);
    bench_regression_lookaround(c);
    bench_regression_unicode(c);
    bench_regression_case_insensitive(c);
    bench_regression_named_captures(c);
    bench_regression_large_text(c);
    bench_regression_regset(c);
    bench_regression_match_at_position(c);
    bench_regression_scanner(c);
    bench_regression_scanner_textmate(c);
}

criterion_group!(benches, bench_onig_bench);
criterion_main!(benches);
