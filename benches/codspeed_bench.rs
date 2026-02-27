// CodSpeed benchmark suite: Ferroni (Rust) pure performance tracking
//
// Run locally: cargo codspeed build -m simulation && cargo codspeed run
// Or via codspeed CLI: codspeed run --mode simulation -- cargo codspeed run

use criterion_codspeed::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

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
// 7. unicode_properties
// ---------------------------------------------------------------------------

fn bench_unicode_properties(c: &mut Criterion) {
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
// 9. named_captures -- extract date fields
// ---------------------------------------------------------------------------

fn bench_named_captures(c: &mut Criterion) {
    let text = b"Event on 2025-12-31 at venue, next on 2026-01-15.";
    let pat = b"(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

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

    let rust_regs: Vec<Box<ferroni::regint::RegexType>> = patterns
        .iter()
        .map(|p| Box::new(rust_compile(p, ONIG_OPTION_NONE)))
        .collect();
    let (rust_set, rc) = onig_regset_new(rust_regs);
    assert!(rc == 0, "Rust regset_new failed: {rc}");
    let mut rust_set = rust_set.unwrap();

    let mut group = c.benchmark_group("regset");

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
// 12. match_at_position -- onig_match at a known offset
// ---------------------------------------------------------------------------

fn bench_match_at_position(c: &mut Criterion) {
    let text = b"xxxx1234abcd";
    let pat = b"\\d+";

    let r_reg = rust_compile(pat, ONIG_OPTION_NONE);

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

    group.finish();
}

// ---------------------------------------------------------------------------
// 13. scanner -- Scanner API
// ---------------------------------------------------------------------------

const SCANNER_PATTERNS: &[&str] = &[
    "Error \\d+",
    "/api/\\w+/\\d+",
    "\\d{4}-\\d{2}-\\d{2}",
    "not found",
    "\\bpage\\b",
];

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
    "(/\\*)(?:\\s*((@)internal)(?=\\s|(\\*/)))?",
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

const SCANNER_TEXT_SHORT: &str = "Error 404: page not found at /api/users/42 on 2025-06-15";

fn make_long_text() -> Vec<u8> {
    let base = b"Error 404: page not found at /api/users/42 on 2025-06-15. ";
    let mut text = Vec::with_capacity(base.len() * 40);
    for _ in 0..40 {
        text.extend_from_slice(base);
    }
    text
}

fn bench_scanner(c: &mut Criterion) {
    let mut group = c.benchmark_group("scanner");

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

    group.finish();
}

// ---------------------------------------------------------------------------
// 14. scanner_textmate -- TextMate-realistic Scanner workload (65 patterns)
// ---------------------------------------------------------------------------

/// Filter TS_EXPRESSION_PATTERNS to only those that compile successfully.
/// This mirrors what shiki-rust does in `build_scanner_for_rule` (<=128 patterns path).
fn valid_ts_patterns() -> Vec<&'static str> {
    TS_EXPRESSION_PATTERNS
        .iter()
        .copied()
        .filter(|p| Scanner::new(&[*p]).is_ok())
        .collect()
}

fn bench_scanner_textmate(c: &mut Criterion) {
    let patterns = valid_ts_patterns();
    let pattern_count = patterns.len();
    let mut group = c.benchmark_group("scanner-textmate");

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
// 15. idiomatic API -- Regex::new / find / captures
// ---------------------------------------------------------------------------

fn bench_idiomatic_api(c: &mut Criterion) {
    use ferroni::prelude::*;

    let mut group = c.benchmark_group("idiomatic_api");

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

// ---------------------------------------------------------------------------
// 16. scanner_css -- CSS grammar patterns (Unicode character class heavy)
//     See https://github.com/sebastian-software/ferroni/issues/10
// ---------------------------------------------------------------------------

/// Patterns extracted from the CSS TextMate grammar (tm-grammars).
/// These are representative of the regex sets compiled during CSS tokenization.
/// Key cost drivers: `\w` (expands to full Unicode word chars), `[-\w]+`, and
/// broad character classes that force CClassMb/CClassMix matching.
const CSS_PATTERNS: &[&str] = &[
    // Property names (Unicode-aware \w)
    r"[-a-zA-Z_][-\w]*",
    // Property values with word chars
    r"[-\w]+",
    // Numeric values
    r"[-+]?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?",
    // Units
    r"(?:em|ex|ch|rem|vw|vh|vmin|vmax|cm|mm|in|px|pt|pc|%|s|ms|deg|rad|grad|turn|Hz|kHz|dpi|dpcm|dppx|fr)\b",
    // Hex colors
    r"#(?:[0-9a-fA-F]{3,4}){1,2}\b",
    // String (double-quoted)
    r#""[^"\\]*(?:\\.[^"\\]*)*""#,
    // String (single-quoted)
    r"'[^'\\]*(?:\\.[^'\\]*)*'",
    // URL function
    r"url\(",
    // CSS functions
    r"(?:calc|min|max|clamp|var|env|rgb|rgba|hsl|hsla|hwb|lab|lch|oklch|oklab|color|linear-gradient|radial-gradient|conic-gradient)\(",
    // Important
    r"!\s*important\b",
    // Selectors: class/id
    r"[.#][-\w]+",
    // Pseudo-classes/elements
    r"::?[-\w]+(?:\([^)]*\))?",
    // At-rules
    r"@[-\w]+",
    // Combinators and punctuation
    r"[>+~|]",
    r"[{}();,:]",
    // Attribute selectors
    r"\[[-\w]+(?:[~|^$*]?=)?",
    // Comments
    r"/\*",
    r"\*/",
    // Whitespace run (often matched)
    r"\s+",
    // Catch-all identifier (Unicode-aware)
    r"\w+",
];

const CSS_INPUT: &str = "\
.navbar-primary > .nav-item:first-child {\n\
  background-color: oklch(0.65 0.15 250);\n\
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;\n\
  margin: 0.5rem 1rem;\n\
  padding: 12px 24px;\n\
  border: 1px solid #e5e7eb;\n\
  transition: all 150ms cubic-bezier(0.4, 0, 0.2, 1);\n\
  --custom-property: var(--color-primary, #3b82f6);\n\
  width: calc(100% - 2rem);\n\
}\n\
\n\
@media (min-width: 768px) and (prefers-color-scheme: dark) {\n\
  .navbar-primary > .nav-item:hover {\n\
    background-color: oklch(0.45 0.12 250);\n\
    color: #f9fafb;\n\
    transform: translateY(-1px);\n\
    box-shadow: 0 4px 6px -1px rgb(0 0 0 / 0.1);\n\
  }\n\
}\n\
";

fn bench_scanner_css(c: &mut Criterion) {
    let patterns: Vec<&str> = CSS_PATTERNS
        .iter()
        .copied()
        .filter(|p| Scanner::new(&[*p]).is_ok())
        .collect();
    let pattern_count = patterns.len();
    let mut group = c.benchmark_group("scanner-css");

    // compile: measure Scanner::new for CSS patterns
    {
        let label = format!("compile_{pattern_count}_patterns");
        group.bench_function(&label, |b| {
            b.iter(|| {
                let scanner = Scanner::new(black_box(&patterns)).unwrap();
                black_box(scanner);
            });
        });
    }

    // single match from position 0
    {
        let onig_str = OnigString::new(CSS_INPUT);
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

    // tokenize: scan entire CSS input token-by-token
    {
        let onig_str = OnigString::new(CSS_INPUT);
        let mut scanner = Scanner::new(&patterns).unwrap();
        let input_len = CSS_INPUT.encode_utf16().count();

        let label = format!("{pattern_count}_patterns_tokenize");
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

    // tokenize repeated: 10x CSS input to amortize setup
    {
        let repeated: String = CSS_INPUT.repeat(10);
        let onig_str = OnigString::new(&repeated);
        let mut scanner = Scanner::new(&patterns).unwrap();
        let input_len = repeated.encode_utf16().count();

        let label = format!("{pattern_count}_patterns_tokenize_10x");
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

    // isolated: just \w+ matching against CSS text (isolate Unicode overhead)
    {
        let onig_str = OnigString::new(CSS_INPUT);
        let mut scanner = Scanner::new(&[r"\w+", r"\s+", r"[^\w\s]+"]).unwrap();
        let input_len = CSS_INPUT.encode_utf16().count();

        group.bench_function("word_class_tokenize", |b| {
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
    bench_idiomatic_api,
    bench_scanner_css,
);
criterion_main!(benches);
