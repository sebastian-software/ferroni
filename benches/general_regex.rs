//! General regex tasks, independent of the TextMate scanner.
//!
//! Compile once, then validate a mixed batch or collect every match/capture.
//! The redaction case uses the same output builder for all three engines.

use std::fmt::Write;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use ferroni::ffi::{self, CRegex, CRegion};
use ferroni::oniguruma::{ONIG_OPTION_NONE, OnigRegion};
use ferroni::regexec::onig_search;
use ferroni::regint::RegexType;
use regex::bytes::Regex;

use super::{assert_same_match, c_compile, configure_battle_group, regex_compile, rust_compile};

const EMAIL: &str = r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}";
const BATCH_REPEATS: usize = 8;
const TEXT_RECORDS: usize = 64;

type Captures = Vec<(i32, i32)>;
type Trace = Vec<Captures>;

struct ValidationCase {
    name: &'static str,
    pattern: String,
    inputs: [(&'static str, bool); 8],
}

fn validation_cases() -> Vec<ValidationCase> {
    vec![
        ValidationCase {
            name: "email_validation",
            // A deliberately simplified ASCII shape check, not RFC validation.
            pattern: format!(r"\A{EMAIL}\z"),
            inputs: [
                ("alice@example.com", true),
                ("first.last+tag@sub.example.org", true),
                ("a_b@service.test", true),
                ("USER42@EXAMPLE.NET", true),
                ("missing-at.example.com", false),
                ("user@example", false),
                ("user name@example.com", false),
                ("alice@example.com!", false),
            ],
        },
        ValidationCase {
            name: "uuid_validation",
            pattern:
                r"\A[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}\z"
                    .into(),
            inputs: [
                ("550e8400-e29b-41d4-a716-446655440000", true),
                ("550E8400-E29B-41D4-A716-446655440000", true),
                ("00000000-0000-0000-0000-000000000000", true),
                ("ffffffff-ffff-ffff-ffff-ffffffffffff", true),
                ("550e8400e29b41d4a716446655440000", false),
                ("550e8400-e29b-41d4-a716-44665544000g", false),
                ("550e8400-e29b-41d4-a716-44665544000", false),
                ("550e8400-e29b-41d4-a716-446655440000!", false),
            ],
        },
        ValidationCase {
            name: "number_validation",
            pattern: r"\A[+-]?(?:[0-9]+(?:\.[0-9]+)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?\z".into(),
            inputs: [
                ("42", true),
                ("-12345.6789", true),
                ("+.125", true),
                ("6.022e+23", true),
                ("", false),
                ("1.2.3", false),
                ("12e+", false),
                ("12345678901234567890.123456789x", false),
            ],
        },
    ]
}

/// These fixtures deliberately exclude empty matches. That lets all engines
/// use the same byte-offset iteration without differing empty-match policies.
fn collect_matches(text_len: usize, mut search: impl FnMut(usize) -> Option<Captures>) -> Trace {
    let mut trace = Vec::new();
    let mut start = 0;
    while start < text_len {
        let Some(captures) = search(start) else {
            break;
        };
        let (beg, end) = captures[0];
        assert!(beg >= start as i32 && end > beg);
        start = end as usize;
        trace.push(captures);
    }
    trace
}

fn rust_trace(regex: &RegexType, text: &[u8]) -> Trace {
    let mut region = Some(OnigRegion::new());
    collect_matches(text.len(), |start| {
        let (pos, next_region) = onig_search(
            regex,
            text,
            text.len(),
            start,
            text.len(),
            region.take(),
            ONIG_OPTION_NONE,
        );
        region = next_region;
        assert!(pos >= -1, "Ferroni search failed: {pos}");
        (pos >= 0).then(|| {
            let region = region.as_ref().unwrap();
            region
                .beg
                .iter()
                .copied()
                .zip(region.end.iter().copied())
                .take(region.num_regs as usize)
                .collect()
        })
    })
}

fn c_trace(regex: &CRegex, text: &[u8]) -> Trace {
    let mut region = CRegion::new();
    collect_matches(text.len(), |start| {
        let pos = regex.search(
            text,
            start,
            text.len(),
            Some(&mut region),
            ffi::ONIG_OPTION_NONE,
        );
        assert!(pos >= -1, "C search failed: {pos}");
        (pos >= 0).then(|| region.capture_ranges())
    })
}

/// A bounded characterization matrix for Unicode range lookup changes.
/// These are synthetic workloads, not a claim about application frequencies.
/// Record boundaries keep adjacent matches from joining when repeated.
pub fn bench_unicode_classes(c: &mut Criterion) {
    let cases = [
        (
            "letters_ascii",
            r"\p{L}+",
            "The quick brown fox jumps over 123!\n",
            6,
        ),
        (
            "letters_latin",
            r"\p{L}+",
            "café résumé naïve Ångström Straße 123!\n",
            5,
        ),
        (
            "letters_mixed",
            r"\p{L}+",
            "Hello Κόσμε Привет 世界 مرحبا café e\u{301}!\n",
            7,
        ),
        (
            "words_mixed",
            r"\w+",
            "café_42 κόσμος Привет_7 世界 مرحبا e\u{301} 123!\n",
            7,
        ),
        (
            "identifiers_mixed",
            r"[\p{L}_][\p{L}\p{M}\p{N}_]*",
            "_name café42 κόσμος_7 имя_2 变量 مرحبا e\u{301} 123!\n",
            7,
        ),
        (
            "combining_marks",
            r"\p{M}+",
            "cafe\u{301} a\u{308}\u{301} क\u{93f} س\u{64e}!\n",
            4,
        ),
        (
            "decimal_digits",
            r"\p{Nd}+",
            "id=123٤٥٦ ७८९ ０１２ and text\n",
            3,
        ),
        (
            "unicode_spaces",
            r"\p{White_Space}+",
            "one two\tthree\u{a0}four\u{2003}five\u{3000}six\n",
            6,
        ),
        (
            "greek",
            r"\p{Greek}+",
            "Hello Κόσμε Προβολή Привет 世界 café!\n",
            2,
        ),
        (
            "cyrillic",
            r"\p{Cyrillic}+",
            "Hello Привет мир Κόσμε 世界 café!\n",
            2,
        ),
        (
            "han",
            r"\p{Han}+",
            "Hello 世界 中文 Κόσμε Привет café!\n",
            2,
        ),
        (
            "greek_no_match",
            r"\p{Greek}+",
            "Hello Привет мир 世界 مرحبا café 123!\n",
            0,
        ),
        (
            "letters_no_match",
            r"\p{L}+",
            "123 ٤٥٦ ७८९ ０１２ 😀 🎉 +-=!?\n",
            0,
        ),
        (
            "negated_letters",
            r"[^\p{L}\p{M}\s]+",
            "café42, Κόσμε!? 世界99 / مرحبا_7\n",
            5,
        ),
    ];
    let mut group = c.benchmark_group("unicode_classes");
    configure_battle_group(&mut group);
    for (name, pattern, record, matches_per_record) in cases {
        let rust = rust_compile(pattern.as_bytes(), ONIG_OPTION_NONE);
        let c_regex = c_compile(pattern.as_bytes(), ffi::ONIG_OPTION_NONE);
        for (size, repeats) in [("short", 1), ("long", TEXT_RECORDS)] {
            let text = record.repeat(repeats);
            let expected = c_trace(&c_regex, text.as_bytes());
            assert_eq!(
                expected.len(),
                matches_per_record * repeats,
                "{name}/{size}"
            );
            assert_eq!(
                rust_trace(&rust, text.as_bytes()),
                expected,
                "{name}/{size}"
            );
            eprintln!(
                "WORKLOAD {}",
                serde_json::json!({
                    "name": format!("{name}_{size}"),
                    "pattern": pattern,
                    "record": record,
                    "records": repeats,
                    "bytes": text.len(),
                    "matches": expected.len(),
                    "boundary": "all matches and raw capture bounds; materialized result vectors",
                })
            );
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(BenchmarkId::new("rust", format!("{name}_{size}")), |b| {
                b.iter(|| black_box(rust_trace(&rust, black_box(text.as_bytes()))));
            });
        }
    }
    group.finish();
}

fn regex_trace(regex: &Regex, text: &[u8]) -> Trace {
    let mut locations = regex.capture_locations();
    collect_matches(text.len(), |start| {
        regex
            .captures_read_at(&mut locations, text, start)
            .map(|_| {
                (0..locations.len())
                    .map(|i| {
                        locations
                            .get(i)
                            .map_or((-1, -1), |(beg, end)| (beg as i32, end as i32))
                    })
                    .collect()
            })
    })
}

fn redact(text: &[u8], trace: Trace) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len());
    let mut start = 0;
    for captures in trace {
        let (beg, end) = captures[0];
        output.extend_from_slice(&text[start..beg as usize]);
        output.extend_from_slice(b"[email]");
        start = end as usize;
    }
    output.extend_from_slice(&text[start..]);
    output
}

struct TextCase {
    name: &'static str,
    pattern: &'static str,
    text: String,
    matches: usize,
    first_captures: Vec<&'static str>,
    redacted: Option<String>,
}

fn text_cases() -> Vec<TextCase> {
    let mut log = String::new();
    let mut urls = String::new();
    let mut emails = String::new();
    let mut redacted = String::new();
    for i in 0..TEXT_RECORDS {
        writeln!(
            log,
            "192.0.2.{i} - - [26/Sep/2026:10:00:00 +0000] \"GET /api/items/{i} HTTP/1.1\" 200 1234"
        )
        .unwrap();
        if i % 4 == 0 {
            log.push_str("maintenance message without access-log fields\n");
        }
        let scheme = if i % 2 == 0 { "https" } else { "http" };
        writeln!(
            urls,
            "Record {i}: see {scheme}://example.org/items/{i}?page=2 and plain text without a link."
        )
        .unwrap();
        writeln!(
            emails,
            "Contact user{i}@example.org or support+{i}@service.test; invalid: user{i} at example"
        )
        .unwrap();
        writeln!(
            redacted,
            "Contact [email] or [email]; invalid: user{i} at example"
        )
        .unwrap();
    }
    vec![
        TextCase {
            name: "access_log_captures",
            pattern: r#"(?m)^([0-9.]+) - - \[([^\]]+)\] "([A-Z]+) ([^ ]+) HTTP/[0-9.]+" ([0-9]{3}) ([0-9]+)$"#,
            text: log,
            matches: TEXT_RECORDS,
            first_captures: vec![
                "192.0.2.0",
                "26/Sep/2026:10:00:00 +0000",
                "GET",
                "/api/items/0",
                "200",
                "1234",
            ],
            redacted: None,
        },
        TextCase {
            name: "url_extraction",
            pattern: r"https?://[A-Za-z0-9.-]+(?:/[A-Za-z0-9/_?=&%.-]*)?",
            text: urls,
            matches: TEXT_RECORDS,
            first_captures: vec![],
            redacted: None,
        },
        TextCase {
            name: "unicode_words",
            pattern: r"\p{L}[\p{L}\p{M}]*",
            text: "Grüße Κόσμε Привет مرحبا 世界 cafe\u{301}\n".repeat(TEXT_RECORDS),
            matches: 6 * TEXT_RECORDS,
            first_captures: vec![],
            redacted: None,
        },
        TextCase {
            name: "email_redaction",
            pattern: EMAIL,
            text: emails,
            matches: 2 * TEXT_RECORDS,
            first_captures: vec![],
            redacted: Some(redacted),
        },
    ]
}

pub fn bench_general_regex(c: &mut Criterion) {
    let mut group = c.benchmark_group("general_regex");
    configure_battle_group(&mut group);
    for case in validation_cases() {
        let rust = rust_compile(case.pattern.as_bytes(), ONIG_OPTION_NONE);
        let c = c_compile(case.pattern.as_bytes(), ffi::ONIG_OPTION_NONE);
        let regex = regex_compile(case.pattern.as_bytes(), false);
        let inputs = case.inputs.repeat(BATCH_REPEATS);
        for (text, expected) in &inputs {
            assert_same_match(&rust, &c, text.as_bytes(), case.name);
            assert_eq!(
                super::rust_search(&rust, text.as_bytes(), None).0 >= 0,
                *expected,
                "{}: {text}",
                case.name
            );
            assert_eq!(
                c.search(text.as_bytes(), 0, text.len(), None, ffi::ONIG_OPTION_NONE) >= 0,
                *expected
            );
            assert_eq!(regex.is_match(text.as_bytes()), *expected);
        }
        println!(
            "WORKLOAD {}",
            serde_json::json!({"name":case.name,"pattern":case.pattern,"inputs":inputs.len(),"bytes":inputs.iter().map(|(s,_)| s.len()).sum::<usize>(),"matches":inputs.iter().filter(|(_,valid)| *valid).count(),"boundary":"boolean validation batch; no capture output"})
        );
        group.throughput(Throughput::Elements(inputs.len() as u64));
        group.bench_function(BenchmarkId::new("rust", case.name), |b| {
            b.iter(|| {
                let count = black_box(&inputs)
                    .iter()
                    .filter(|(text, _)| super::rust_search(&rust, text.as_bytes(), None).0 >= 0)
                    .count();
                black_box(count);
            })
        });
        group.bench_function(BenchmarkId::new("c", case.name), |b| {
            b.iter(|| {
                let count = black_box(&inputs)
                    .iter()
                    .filter(|(text, _)| {
                        c.search(text.as_bytes(), 0, text.len(), None, ffi::ONIG_OPTION_NONE) >= 0
                    })
                    .count();
                black_box(count);
            })
        });
        group.bench_function(BenchmarkId::new("regex", case.name), |b| {
            b.iter(|| {
                let count = black_box(&inputs)
                    .iter()
                    .filter(|(text, _)| regex.is_match(text.as_bytes()))
                    .count();
                black_box(count);
            })
        });
    }

    for case in text_cases() {
        let text = case.text.as_bytes();
        let rust = rust_compile(case.pattern.as_bytes(), ONIG_OPTION_NONE);
        let c = c_compile(case.pattern.as_bytes(), ffi::ONIG_OPTION_NONE);
        let regex = regex_compile(case.pattern.as_bytes(), false);
        let expected = rust_trace(&rust, text);
        assert_eq!(expected.len(), case.matches, "{}: match count", case.name);
        assert_eq!(expected, c_trace(&c, text), "{}: C captures", case.name);
        assert_eq!(
            expected,
            regex_trace(&regex, text),
            "{}: regex captures",
            case.name
        );
        assert_eq!(expected[0].len(), case.first_captures.len() + 1);
        for (bounds, value) in expected[0].iter().skip(1).zip(&case.first_captures) {
            assert_eq!(
                &text[bounds.0 as usize..bounds.1 as usize],
                value.as_bytes()
            );
        }
        if let Some(redacted) = &case.redacted {
            assert_eq!(redact(text, expected), redacted.as_bytes());
        }
        println!(
            "WORKLOAD {}",
            serde_json::json!({"name":case.name,"pattern":case.pattern,"records":TEXT_RECORDS,"bytes":text.len(),"matches":case.matches,"boundary":if case.redacted.is_some() {"all matches plus shared redaction output builder"} else {"all matches and raw capture bounds; materialized result vectors"}})
        );
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(BenchmarkId::new("rust", case.name), |b| {
            b.iter(|| {
                let trace = rust_trace(&rust, black_box(text));
                if case.redacted.is_some() {
                    black_box(redact(text, trace));
                } else {
                    black_box(trace);
                }
            })
        });
        group.bench_function(BenchmarkId::new("c", case.name), |b| {
            b.iter(|| {
                let trace = c_trace(&c, black_box(text));
                if case.redacted.is_some() {
                    black_box(redact(text, trace));
                } else {
                    black_box(trace);
                }
            })
        });
        group.bench_function(BenchmarkId::new("regex", case.name), |b| {
            b.iter(|| {
                let trace = regex_trace(&regex, black_box(text));
                if case.redacted.is_some() {
                    black_box(redact(text, trace));
                } else {
                    black_box(trace);
                }
            })
        });
    }
    group.finish();
}
