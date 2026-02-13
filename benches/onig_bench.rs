// Criterion benchmark suite: Ferroni (Rust) vs Oniguruma (C)
//
// Run: cargo bench --features ffi
// Specific group: cargo bench --features ffi -- compile
// HTML report: target/criterion/report/index.html

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::os::raw::c_uint;

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::ffi;
use ferroni::oniguruma::{
    ONIG_OPTION_IGNORECASE, ONIG_OPTION_NONE, OnigOptionType, OnigRegion, OnigSyntaxType,
};
use ferroni::regcomp::onig_new;
use ferroni::regexec::{onig_match, onig_region_new, onig_search};
use ferroni::regset::{OnigRegSet, OnigRegSetLead, onig_regset_new, onig_regset_search};
use ferroni::regsyntax::OnigSyntaxOniguruma;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn rust_compile(pattern: &[u8], option: OnigOptionType) -> ferroni::regint::RegexType {
    onig_new(
        pattern,
        option,
        &ONIG_ENCODING_UTF8,
        &OnigSyntaxOniguruma as *const OnigSyntaxType,
    )
    .expect("Rust compile failed")
}

fn rust_search(
    reg: &ferroni::regint::RegexType,
    text: &[u8],
    region: Option<OnigRegion>,
) -> (i32, Option<OnigRegion>) {
    onig_search(reg, text, text.len(), 0, text.len(), region, ONIG_OPTION_NONE)
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
        ("named_capture", b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})"),
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
        ("ten", b"alpha|beta|gamma|delta|epsilon|zeta|eta|theta|iota|wolverine"),
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
            &r_reg, text, text.len(), 0, text.len(), None, ONIG_OPTION_NONE,
        );
        let c_pos = c_reg.search(text, 0, text.len(), None, ffi::ONIG_OPTION_NONE);
        assert_same_result(r_pos, c_pos, name);

        group.bench_with_input(BenchmarkId::new("rust", name), &text[..], |b, text| {
            b.iter(|| {
                let (pos, _) =
                    onig_search(&r_reg, black_box(text), text.len(), 0, text.len(), None, ONIG_OPTION_NONE);
                black_box(pos);
            });
        });
        group.bench_with_input(BenchmarkId::new("c", name), &text[..], |b, text| {
            let mut region = ffi::CRegion::new();
            b.iter(|| {
                region.clear();
                let pos =
                    c_reg.search(black_box(text), 0, text.len(), Some(&mut region), ffi::ONIG_OPTION_NONE);
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
        b.iter(|| {
            let region = onig_region_new();
            let (pos, _region) = onig_search(
                &r_reg,
                black_box(text),
                text.len(),
                0,
                text.len(),
                Some(region),
                ONIG_OPTION_NONE,
            );
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
        group.bench_with_input(
            BenchmarkId::new("c", &label_10k),
            &text_10k,
            |b, text| {
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
        group.bench_with_input(
            BenchmarkId::new("c", &label_50k),
            &text_50k,
            |b, text| {
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
            },
        );
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
);
criterion_main!(benches);
