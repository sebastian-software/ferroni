use super::*;
use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::oniguruma::*;
use crate::regcomp::onig_new;
use crate::regexec::{
    onig_new_match_param, onig_search, onig_search_with_param,
    onig_set_retry_limit_in_match_of_match_param, shared_limits,
};
use crate::regsyntax::{OnigSyntaxOniguruma, OnigSyntaxPerl};

fn compile(
    pattern: &[u8],
    option: OnigOptionType,
    syntax: &OnigSyntaxType,
    possessify: bool,
) -> Option<RegexType> {
    POSSESSIFY_DISABLED.with(|disabled| disabled.set(!possessify));
    let reg = onig_new(pattern, option, &ONIG_ENCODING_UTF8, syntax);
    POSSESSIFY_DISABLED.with(|disabled| disabled.set(false));
    reg.ok()
}

/// The stars this pass made possessive: the `PopStarChoices` instructions
/// of `fast` beyond those of `plain`, the program compiled without the pass,
/// which has them only after the star of a possessive repeat of one
/// character (C's `STRICT_REAL_REPEAT`).
fn possessive_stars(fast: &RegexType, plain: &RegexType) -> usize {
    let count = |reg: &RegexType| {
        reg.ops
            .iter()
            .filter(|op| op.opcode == OpCode::PopStarChoices)
            .count()
    };
    count(fast) - count(plain)
}

/// `possessive_stars` of a pattern under a syntax.
fn stars_with(pattern: &[u8], syntax: &OnigSyntaxType) -> usize {
    let compiled = |possessify| {
        compile(pattern, ONIG_OPTION_NONE, syntax, possessify)
            .unwrap_or_else(|| panic!("{pattern:?} compiles"))
    };
    possessive_stars(&compiled(true), &compiled(false))
}

fn stars_in(pattern: &str) -> usize {
    stars_with(pattern.as_bytes(), &OnigSyntaxOniguruma)
}

/// A search result: the return value and the capture spans.
type Outcome = (i32, Vec<(i32, i32)>);

fn search(
    reg: &RegexType,
    subject: &[u8],
    start: usize,
    option: OnigOptionType,
    retry_limit: Option<u64>,
) -> Outcome {
    let (r, region) = match retry_limit {
        None => onig_search(
            reg,
            subject,
            subject.len(),
            start,
            subject.len(),
            Some(OnigRegion::new()),
            option,
        ),
        Some(limit) => {
            let mut mp = onig_new_match_param();
            onig_set_retry_limit_in_match_of_match_param(&mut mp, limit);
            onig_search_with_param(
                reg,
                subject,
                subject.len(),
                start,
                subject.len(),
                Some(OnigRegion::new()),
                option,
                &mp,
            )
        }
    };
    let spans = match region {
        Some(region) if r >= 0 => region
            .beg
            .iter()
            .zip(&region.end)
            .take(region.num_regs as usize)
            .map(|(&b, &e)| (b, e))
            .collect(),
        _ => Vec::new(),
    };
    (r, spans)
}

/// `django_url_regex` and `django_email_domain` of the production corpus.
const DJANGO_URL: &str = "(?i)^(?:[a-z0-9.+-]*)://(?:[^\\s:@/]+(?::[^\\s:@/]*)?@)?(?:(?:0|25[0-5]|2[0-4][0-9]|1[0-9]?[0-9]?|[1-9][0-9]?)(?:\\.(?:0|25[0-5]|2[0-4][0-9]|1[0-9]?[0-9]?|[1-9][0-9]?)){3}|\\[[0-9a-f:.]+\\]|([a-z¡-\u{ffff}0-9](?:[a-z¡-\u{ffff}0-9-]{0,61}[a-z¡-\u{ffff}0-9])?(?:\\.(?!-)[a-z¡-\u{ffff}0-9-]{1,63}(?<!-))*\\.(?!-)(?:[a-z¡-\u{ffff}-]{2,63}|xn--[a-z0-9]{1,59})(?<!-)\\.?|localhost))(?::[0-9]{1,5})?(?:[/?#][^\\s]*)?\\z";
const DJANGO_EMAIL_DOMAIN: &str = "(?i)^[a-z¡-\u{ffff}0-9](?:[a-z¡-\u{ffff}0-9-]{0,61}[a-z¡-\u{ffff}0-9])?(?:\\.(?!-)[a-z¡-\u{ffff}0-9-]{1,63}(?<!-))*\\.(?!-)(?:[a-z¡-\u{ffff}-]{2,63}|xn--[a-z0-9]{1,59})(?<!-)\\z";

#[test]
fn stars_become_possessive_where_no_follower_can_start() {
    // The production cases: an optional group before the decisive literal,
    // and a class run before `\z`.
    assert_eq!(stars_in(r"[^\s:@/]+(?::[^\s:@/]*)?@"), 2);
    assert_eq!(
        stars_in(r"(?i)^(?:[a-z0-9.+-]*)://(?:[^\s:@/]+(?::[^\s:@/]*)?@)?"),
        2
    );
    assert_eq!(stars_in(r"(?:[/?#][^\s]*)?\z"), 1);
    assert_eq!(stars_in(DJANGO_URL), 3);
    assert_eq!(stars_in(DJANGO_EMAIL_DOMAIN), 0, "bounded repeats only");
    // Alternations, nested optional parts, groups and their ends.
    assert_eq!(stars_in(r"[a-z]+(?:1|2)"), 1);
    assert_eq!(stars_in(r"[a-z]+(?:1|(?:2)?3)"), 1);
    assert_eq!(stars_in(r"[a-z]+(?:,[a-z]*)*;"), 2);
    assert_eq!(stars_in(r"(?:([a-z]+)|0)(?=x|1)\b1"), 1);
    assert_eq!(stars_in(r"\w+(?:-|\+|)\.?:"), 1);
    assert_eq!(stars_in(r"\w+(?:-|_|)\.?:"), 0);
    assert_eq!(stars_in(r"(?:a+|b)(?:c)?d"), 1);
    // End anchors: `$` and `\Z` only for a body without the newline.
    assert_eq!(stars_in(r"[a-z]+$"), 1);
    assert_eq!(stars_in(r"[a-z]*(?: x)?\Z"), 1);
    assert_eq!(stars_in(r"[^ ]+\z"), 1);
    assert_eq!(stars_in(r"[^ ]+$"), 0);
    assert_eq!(stars_in(r"[^ ]+\Z"), 0);
    assert_eq!(stars_in(r"\W+$"), 0);
    // A follower that may start with what the body matches.
    assert_eq!(stars_in(r"[a-z]+(?:x)?1"), 0);
    assert_eq!(stars_in(r"[a-z]+(?:1|x)"), 0);
    assert_eq!(stars_in(r"[a-z]+(?:1)?"), 0);
    assert_eq!(stars_in(r"[a-z]+(?:1)?(?:2|)"), 0);
    assert_eq!(stars_in(r"\s+(?!\S)"), 0);
    // Out of scope.
    assert_eq!(stars_in(r"[a-z]+?(?:1)?2"), 0, "lazy");
    assert_eq!(stars_in(r"[a-z]{1,9}(?:1)?2"), 0, "bounded");
    assert_eq!(stars_in(r"(a)[a-z]+(?:1)?2\1"), 0, "back reference");
    assert_eq!(stars_in(r"(?<n>[a-z]+(?:1)?2)\g<n>"), 0, "call");
    assert_eq!(stars_in(r"[a-z]+(?:1)?(?<=a)2"), 0, "look-behind");
    assert_eq!(stars_in(r"[a-z]+(?:1)?(?{foo})2"), 0, "callout");
    assert_eq!(stars_in(r"[a-z]+(?:1)?(?~ab)2"), 0, "absent operator");
    assert_eq!(
        stars_in(r"(?~ab)[a-z]+(?:1)?2"),
        1,
        "after an absent operator"
    );
    assert_eq!(stars_in(r"(?:é|ü)+(?:1)?2"), 0, "not a star instruction");
    assert_eq!(stars_in(r"(?=[a-z]+(?:1)?2)"), 0, "inside a look-ahead");
    assert_eq!(stars_in(r"(?>[a-z]+(?:1)?2)"), 0, "inside an atomic group");
    // C's own possessification (`tune_next`) keeps its form.
    assert_eq!(stars_in(r"[a-z]+1"), 0);
}

/// Hand-written cases where a shorter run would be the one that matches,
/// had the walk accepted the follower.
#[test]
fn shorter_runs_that_match_keep_the_star_greedy() {
    let _limits = shared_limits();
    for (pattern, subject, possessive) in [
        // The next pass through the body starts inside the run.
        (r"(?:a[a-z]*|x1)*;", "aax1;", 0),
        (r"(?:,[a-z]*|x1)*;", ",ax1;", 0),
        (r"(?:,[a-z]*|;1)*;", ",ab;", 1),
        // `$` before a newline the run took; `\Z` before the final one.
        (r"[^ ]+(?: x)?$", "ab\ncd e", 0),
        (r"[^a]*(?:a)?\Z", "b\n", 0),
        (r"[^a\n]*(?:a)?\Z", "b\n", 1),
        // An empty branch, then a literal the class contains.
        (r"[0-9a-f]+(?::|)f", "12ff", 0),
        (r"[0-9a-f]+(?::|)g", "12fg", 1),
        // Case-insensitive followers.
        (r"(?i)[a-z]+(?:1)?K", "abk", 0),
        (r"(?i)[a-j]+(?:1)?K", "abk", 1),
        (r"[a-j]+(?i:(?:1)?\x{212a})", "abk", 1),
        // Multibyte classes and followers.
        (r"[à-ÿ]+(?:1)?é", "àéé", 0),
        (r"[à-ÿ]+(?:1)?Ā", "àéĀ", 1),
        (r"[^é]+(?:x)?é", "abé", 0),
        (r"[^é]+(?:1)?é", "a1é", 0),
        (r"[^é1]+(?:1)?é", "abé", 1),
    ] {
        let fast = compile(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &OnigSyntaxOniguruma,
            true,
        )
        .unwrap();
        let plain = compile(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &OnigSyntaxOniguruma,
            false,
        )
        .unwrap();
        assert_eq!(possessive_stars(&fast, &plain), possessive, "{pattern}");
        let subject = subject.as_bytes();
        for start in 0..=subject.len() {
            let expected = search(&plain, subject, start, ONIG_OPTION_NONE, None);
            assert_eq!(
                search(&fast, subject, start, ONIG_OPTION_NONE, None),
                expected,
                "{pattern} on {subject:?} from {start}"
            );
            #[cfg(feature = "ffi")]
            {
                let c = crate::ffi::CRegex::new(pattern.as_bytes(), 0).unwrap();
                let mut region = crate::ffi::CRegion::new();
                let r = c.search(subject, start, subject.len(), Some(&mut region), 0);
                let spans = if r >= 0 {
                    region.capture_ranges()
                } else {
                    Vec::new()
                };
                assert_eq!(
                    (r, spans),
                    expected,
                    "C: {pattern} on {subject:?} from {start}"
                );
            }
        }
    }
}

/// Possessive stars must not change any result: compare with the program
/// that keeps every choice, and with C, over generated patterns of the
/// covered shapes, options, and subjects with multibyte, malformed and
/// newline characters.
#[test]
fn generated_patterns_match_the_greedy_program_and_c() {
    let _limits = shared_limits();
    let mut rng = Rng(0x5eed_1234_abcd_0001);
    let subjects = subjects(&mut rng);
    let options = [
        ONIG_OPTION_NONE,
        ONIG_OPTION_IGNORECASE,
        ONIG_OPTION_SINGLELINE,
        ONIG_OPTION_MULTILINE,
        ONIG_OPTION_FIND_LONGEST,
        ONIG_OPTION_FIND_NOT_EMPTY,
    ];
    let search_options = [
        ONIG_OPTION_NONE,
        ONIG_OPTION_NOTEOL,
        ONIG_OPTION_MATCH_WHOLE_STRING,
    ];
    let mut patterns = 0;
    let mut possessive = 0;
    let mut comparisons = 0u64;
    // Absent operators and `\K` beside possessive stars, then generated ones.
    let fixed = [
        r"(?~ab)[a-z]+(?:1)?2",
        r"(?~|ab|[a-z]+(?:1)?:)",
        r"(?~|ab|[a-z]+)(?:1)?:",
        r"(?~|:)[^1\n]+(?:1)?$",
        r"(?~|:)[a-z]+(?:,x)?(?~|)\z",
        r"[a-z]+\K(?::[0-9]*)?@",
        r"(?:a\K[^@\n1]*(?:1)?@)+",
    ];
    for index in 0..fixed.len() + 700 {
        let pattern = match fixed.get(index) {
            Some(pattern) => pattern.to_string(),
            None => generate_pattern(&mut rng),
        };
        let option = options[rng.below(options.len())];
        let search_option = search_options[rng.below(search_options.len())];
        let fast = compile(pattern.as_bytes(), option, &OnigSyntaxOniguruma, true);
        let plain = compile(pattern.as_bytes(), option, &OnigSyntaxOniguruma, false);
        let (Some(fast), Some(plain)) = (fast, plain) else {
            continue;
        };
        patterns += 1;
        if possessive_stars(&fast, &plain) > 0 {
            possessive += 1;
        }
        #[cfg(feature = "ffi")]
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), option.bits()).unwrap();
        for subject in &subjects {
            for start in [0, 1, subject.len() / 2, subject.len()] {
                if start > subject.len() {
                    continue;
                }
                let context = format!(
                    "{pattern} ({option:?}, {search_option:?}) on {subject:?} from {start}"
                );
                let expected = search(&plain, subject, start, search_option, None);
                let result = search(&fast, subject, start, search_option, None);
                // Where the greedy program stops at the default retry limit,
                // the possessive one may finish (ADR-017); C decides below.
                if expected.0 != ONIGERR_RETRY_LIMIT_IN_MATCH_OVER {
                    assert_eq!(result, expected, "{context}");
                }
                // C steps over malformed UTF-8 differently in places this
                // change does not touch; the greedy program covers those.
                #[cfg(feature = "ffi")]
                if std::str::from_utf8(subject).is_ok() {
                    let mut region = crate::ffi::CRegion::new();
                    let r = c.search(
                        subject,
                        start,
                        subject.len(),
                        Some(&mut region),
                        search_option.bits(),
                    );
                    // Where C stops at its default retry limit, Ferroni may
                    // finish (ADR-017); the greedy program checks the result.
                    if r != ONIGERR_RETRY_LIMIT_IN_MATCH_OVER {
                        let spans = if r >= 0 {
                            region.capture_ranges()
                        } else {
                            Vec::new()
                        };
                        assert_eq!((r, spans), result, "C: {context}");
                    }
                }
                comparisons += 1;
            }
        }
    }
    assert!(patterns > 600, "only {patterns} patterns compiled");
    assert!(
        possessive * 3 > patterns,
        "only {possessive} of {patterns} patterns have a possessive star"
    );
    eprintln!("{comparisons} comparisons, {possessive} of {patterns} patterns possessive");
}

/// Perl's `$` (`\Z` unless multi-line) and its multi-line `$` read as the
/// anchors they compile to.
#[test]
fn perl_syntax_end_anchors_keep_their_results() {
    let _limits = shared_limits();
    let mut rng = Rng(0x0dd_ba11);
    let subjects = subjects(&mut rng);
    for pattern in [
        r"[a-z]+(?::[0-9]*)?$",
        r"(?m)[a-z]+(?::[0-9]*)?$",
        r"[^\s]+(?: [a-z]+)?$",
        r"(?m)[^ ]+(?:,x)?$",
        r"(?s)[^:]+(?::)?$",
        r"[a-z]*(?:é)?\Z",
    ] {
        let fast = compile(pattern.as_bytes(), ONIG_OPTION_NONE, &OnigSyntaxPerl, true).unwrap();
        let plain = compile(pattern.as_bytes(), ONIG_OPTION_NONE, &OnigSyntaxPerl, false).unwrap();
        for subject in &subjects {
            for start in 0..=subject.len() {
                assert_eq!(
                    search(&fast, subject, start, ONIG_OPTION_NONE, None),
                    search(&plain, subject, start, ONIG_OPTION_NONE, None),
                    "{pattern} on {subject:?} from {start}"
                );
            }
        }
    }
    assert_eq!(stars_with(b"[a-z]+(?::[0-9]*)?$", &OnigSyntaxPerl), 2);
    assert_eq!(stars_with(b"(?m)[^ ]+(?:,x)?$", &OnigSyntaxPerl), 0);
}

/// ADR-017: the possessive star leaves out backtracks. Where C stops at the
/// retry limit, Ferroni returns C's error or C's result without the limit;
/// where C completes, so does Ferroni, with the same result.
#[test]
fn retry_limits_follow_adr_017() {
    let _limits = shared_limits();
    let pattern = r"^(?:[^\s:@/]+(?::[^\s:@/]*)?@)?[a-z]+\z";
    let fast = compile(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        &OnigSyntaxOniguruma,
        true,
    )
    .unwrap();
    let plain = compile(
        pattern.as_bytes(),
        ONIG_OPTION_NONE,
        &OnigSyntaxOniguruma,
        false,
    )
    .unwrap();
    assert_eq!(possessive_stars(&fast, &plain), 3);
    let subject = format!("{}!", "a".repeat(5_000));
    let subject = subject.as_bytes();
    // The greedy program gives the run back one character at a time.
    assert_eq!(
        search(&plain, subject, 0, ONIG_OPTION_NONE, Some(1_000)).0,
        ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
    );
    assert_eq!(
        search(&fast, subject, 0, ONIG_OPTION_NONE, Some(1_000)),
        search(&plain, subject, 0, ONIG_OPTION_NONE, None)
    );
    #[cfg(feature = "ffi")]
    {
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), 0).unwrap();
        let c_limited = c.search_with_retry_limit(subject, 0, subject.len(), None, 0, 1_000);
        assert_eq!(c_limited, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER);
        let c_unlimited = c.search(subject, 0, subject.len(), None, 0);
        assert_eq!(
            search(&fast, subject, 0, ONIG_OPTION_NONE, Some(1_000)).0,
            c_unlimited
        );
    }

    let mut rng = Rng(0x00ad_0017);
    let subjects = subjects(&mut rng);
    for _ in 0..150 {
        let pattern = generate_pattern(&mut rng);
        let Some(fast) = compile(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &OnigSyntaxOniguruma,
            true,
        ) else {
            continue;
        };
        let plain = compile(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &OnigSyntaxOniguruma,
            false,
        )
        .unwrap();
        if possessive_stars(&fast, &plain) == 0 {
            continue;
        }
        #[cfg(feature = "ffi")]
        let c = crate::ffi::CRegex::new(pattern.as_bytes(), 0).unwrap();
        for subject in &subjects {
            for limit in [1, 2, 3, 8, 50] {
                let unlimited = search(&fast, subject, 0, ONIG_OPTION_NONE, None);
                let limited = search(&fast, subject, 0, ONIG_OPTION_NONE, Some(limit));
                let context = format!("{pattern} on {subject:?} at limit {limit}");
                #[cfg(feature = "ffi")]
                if std::str::from_utf8(subject).is_ok() {
                    let mut region = crate::ffi::CRegion::new();
                    let r = c.search_with_retry_limit(
                        subject,
                        0,
                        subject.len(),
                        Some(&mut region),
                        0,
                        limit,
                    );
                    if r == ONIGERR_RETRY_LIMIT_IN_MATCH_OVER {
                        assert!(
                            limited.0 == r || limited == unlimited,
                            "C stops, Ferroni gives a third answer: {context}"
                        );
                    } else {
                        let spans = if r >= 0 {
                            region.capture_ranges()
                        } else {
                            Vec::new()
                        };
                        assert_eq!(limited, (r, spans), "C completes: {context}");
                    }
                }
                // Without C: a limit error or the unlimited result.
                assert!(
                    limited.0 == ONIGERR_RETRY_LIMIT_IN_MATCH_OVER || limited == unlimited,
                    "{context}"
                );
            }
        }
    }
}

/// The choices are popped, not voided: a possessive star leaves no stack
/// entry behind, so a repeated group needs no more stack than the greedy
/// program.
#[test]
fn possessive_stars_leave_no_stack_entries() {
    use crate::regexec::onig_set_match_stack_limit_size_of_match_param;

    let _limits = shared_limits();
    let pattern = br"(?:[a-z]+(?:,x)?;)*z";
    let fast = compile(pattern, ONIG_OPTION_NONE, &OnigSyntaxOniguruma, true).unwrap();
    let plain = compile(pattern, ONIG_OPTION_NONE, &OnigSyntaxOniguruma, false).unwrap();
    assert_eq!(possessive_stars(&fast, &plain), 1);
    for unit in ["a;", "abc;"] {
        let subject = format!("{}z", unit.repeat(2_000));
        let run = |reg: &RegexType, stack: u32| {
            let mut mp = onig_new_match_param();
            onig_set_match_stack_limit_size_of_match_param(&mut mp, stack);
            onig_search_with_param(
                reg,
                subject.as_bytes(),
                subject.len(),
                0,
                subject.len(),
                None,
                ONIG_OPTION_NONE,
                &mp,
            )
            .0
        };
        let least = |reg: &RegexType| (1..=10_000).find(|&stack| run(reg, stack) == 0);
        let greedy = least(&plain).expect("the greedy program matches");
        assert!(least(&fast).is_some_and(|stack| stack <= greedy), "{unit}");
    }
}

/// A deterministic generator (xorshift64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

/// Bodies of the stars: classes with and without newline, negated,
/// multibyte and ASCII-only, the word type and single bytes.
const BODIES: &[&str] = &[
    r"[a-z]",
    r"[^\s:@/]",
    r"[^a]",
    r"\w",
    "a",
    "x",
    "[é-ü]",
    "[^é]",
    r"[a-c\n]",
    "[0-9]",
    r"[^\n]",
    r"[\x{100}-\x{2000}]",
    r"[^x\n]",
    r"\d",
    "[:@]",
    "[^ ]",
    "[a-zé]",
    r"[^\x{100}]",
    r"\s",
    "[A-Z]",
];

/// Single-character atoms of followers.
const ATOMS: &[&str] = &[
    ":", "@", "a", "x", "é", "Ā", r"\n", "1", "[0-9]", "[:@]", "[^a-z]", r"\s", r"\w", r"\W", ".",
    " ", ";", "Z",
];

fn follower_element(rng: &mut Rng, depth: u32) -> String {
    let atom = rng.pick(ATOMS);
    if depth >= 2 {
        return atom.to_string();
    }
    let seq = |rng: &mut Rng| follower_seq(rng, depth + 1, 2);
    match rng.below(22) {
        0..=3 => atom.to_string(),
        4 => format!("(?:{})?", seq(rng)),
        5 => format!("(?:{}|{})", seq(rng), seq(rng)),
        6 => format!("(?:{}|)", seq(rng)),
        7 => format!("({})", seq(rng)),
        8 => format!("{atom}*"),
        9 => format!("{atom}+"),
        10 => format!("(?:{})*", seq(rng)),
        11 => "$".to_string(),
        12 => r"\z".to_string(),
        13 => r"\Z".to_string(),
        14 => rng.pick(&[r"\b", r"\B", "^", r"\A", r"\G"]).to_string(),
        15 => format!("(?={atom})"),
        16 => format!("(?!{atom})"),
        17 => format!("(?<={atom})"),
        18 => format!("(?i:{})", seq(rng)),
        19 => format!("(?>{})", seq(rng)),
        20 => format!("(?:{}){{1,2}}", seq(rng)),
        _ => format!("{}?", rng.pick(ATOMS)),
    }
}

fn follower_seq(rng: &mut Rng, depth: u32, max: usize) -> String {
    (0..1 + rng.below(max))
        .map(|_| follower_element(rng, depth))
        .collect()
}

fn generate_pattern(rng: &mut Rng) -> String {
    let mut star = format!("{}{}", rng.pick(BODIES), rng.pick(&["*", "+"]));
    if rng.below(4) == 0 {
        star = format!("({star})");
    }
    let follow = follower_seq(rng, 0, 3);
    let tail = rng.pick(&["", ";", "$", r"\z", "@", "a", ""]);
    let prefix = rng.pick(&["", "", "^", "a", "(?:x|y)", r"\b"]);
    // The star directly, at the end of a group, of a branch, or of a
    // repeated body, which another pass may continue.
    match rng.below(8) {
        0 => format!("{prefix}(?:{star}{follow})*{tail}"),
        1 => format!("{prefix}(?:{star}{follow}|{})+{tail}", rng.pick(ATOMS)),
        2 => format!("{prefix}(?:{star}){follow}{tail}"),
        3 => format!("{prefix}(?:{star}|{}){follow}{tail}", rng.pick(ATOMS)),
        4 => format!("{prefix}(?:{follow}{star})+{follow}{tail}"),
        5 => format!(
            "{prefix}(?:{}{star}|{}{})*{tail}",
            rng.pick(ATOMS),
            rng.pick(ATOMS),
            rng.pick(ATOMS)
        ),
        6 => format!("{prefix}(?:{follow}{star}){{1,3}}{follow}{tail}"),
        _ => format!("{prefix}{star}{follow}{tail}"),
    }
}

fn subjects(rng: &mut Rng) -> Vec<Vec<u8>> {
    let alphabet: &[&[u8]] = &[
        b"a",
        b"b",
        b"x",
        b"Z",
        b":",
        b"@",
        b" ",
        b"\n",
        b"1",
        b";",
        b"/",
        "é".as_bytes(),
        "ü".as_bytes(),
        "Ā".as_bytes(),
        b"\xff",
        b"\xc3",
        b"\x80",
    ];
    let mut subjects: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"a".to_vec(),
        b"\n".to_vec(),
        b"ab\n".to_vec(),
        b"abc:de@fg".to_vec(),
        b"abc:de".to_vec(),
        b"user:pass@host x".to_vec(),
        "aé:1@\n".as_bytes().to_vec(),
        b"aaaa;aa;z".to_vec(),
        b"ab\nab\n".to_vec(),
        b"a\xffb:@".to_vec(),
        b"Zz1 Ab\n".to_vec(),
    ];
    for _ in 0..40 {
        let len = rng.below(9);
        subjects.push(
            (0..len)
                .flat_map(|_| alphabet[rng.below(alphabet.len())].to_vec())
                .collect(),
        );
    }
    subjects
}
