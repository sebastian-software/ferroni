#![no_main]

use ferroni::encodings::utf8::ONIG_ENCODING_UTF8;
use ferroni::oniguruma::{
    ONIG_OPTION_CAPTURE_GROUP, ONIG_OPTION_NONE, ONIG_OPTION_NOT_BEGIN_POSITION,
    ONIG_OPTION_NOT_BEGIN_STRING, ONIG_OPTION_NOT_END_STRING, ONIGERR_RETRY_LIMIT_IN_MATCH_OVER,
};
use ferroni::regcomp::onig_new;
use ferroni::regset::{OnigRegSetLead, onig_regset_new, onig_regset_search};
use ferroni::regsyntax::OnigSyntaxOniguruma;
use ferroni::scanner::{OnigString, Scanner, ScannerConfig, ScannerFindOptions, ScannerMatch};
use libfuzzer_sys::arbitrary::Unstructured;
use libfuzzer_sys::fuzz_target;
use std::time::{Duration, Instant};

const MAX_INPUT_BYTES: usize = 4 * 1024;
const MAX_PATTERNS: usize = 6;
const MAX_TEXT_BYTES: usize = 64;
/// Nodes one generated pattern may hold.
const PATTERN_BUDGET: u32 = 14;
/// Nesting of groups, quantifiers, looks and conditionals.
const MAX_DEPTH: u32 = 3;
/// Comparing stops after this long, so that a case of many offsets or
/// slow searches does not turn into a libFuzzer timeout.
const TIME_BUDGET: Duration = Duration::from_secs(1);
/// A search without the pre-filter that takes longer than this ends its
/// case before the scanner with the pre-filter is asked: a nested
/// quantifier over a run of its own character spends the default retry
/// limit on one attempt (ADR-008 keeps the pre-filter off under any other
/// limit), seconds in an instrumented build, and the search with the
/// pre-filter would run that attempt again in its self-check, as would the
/// oracle of `retry_limit_reached`.
const SLOW_SEARCH: Duration = Duration::from_millis(200);
/// Calls with the same start and a stable id, so that the cache route
/// probes its per-regex path (`ROUTE_PROBE_EVERY`).
const REPEATED_CALLS: usize = 3;
const STRING_ID: u64 = 7;

// The DFA pre-filter of the scanner's RegSet search (ADR-008) is sound only
// while every seek is a superset of its pattern and the search loop
// reproduces the position-lead search's decisions. This target compares a
// scanner with the pre-filter against one without it: the same patterns,
// the same text, every byte offset (interior ones included), every find
// option, and the cache and UTF-16 routes. Any difference is a bug.
//
// The first byte picks how the rest is read. Zero: a generator walks the
// bytes and writes patterns over the constructs the seek approximates
// (negated and nested classes, folds, looks, back references, calls,
// conditionals, the absent operator, anchors) and a text over an alphabet
// of ASCII, newlines, folding and non-ASCII characters and a few chosen
// substrings, so that mutation reaches those shapes. Anything else: raw
// mode, where NUL-separated patterns precede the text after the last NUL,
// the layout of the seeds `write_fuzz_seeds` in
// tests/compat_prefilter_differential.rs writes from the compat pairs and
// the review regressions.
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let Some((&mode, rest)) = data.split_first() else {
        return;
    };
    let (patterns, text) = if mode == 0 {
        generated(rest)
    } else {
        raw(rest)
    };
    let patterns: Vec<&str> = patterns.iter().map(String::as_str).collect();
    compare(&patterns, &text);
});

/// Raw mode: the patterns up to the last NUL, separated by NULs, then the
/// text.
fn raw(data: &[u8]) -> (Vec<String>, String) {
    let Some(split) = data.iter().rposition(|byte| *byte == 0) else {
        return (Vec::new(), String::new());
    };
    let (pattern_bytes, text_bytes) = data.split_at(split);
    let patterns = pattern_bytes
        .split(|byte| *byte == 0)
        .take(MAX_PATTERNS)
        .map(|pattern| String::from_utf8_lossy(pattern).into_owned())
        .collect();
    let text = String::from_utf8_lossy(&text_bytes[1..]).into_owned();
    (patterns, text)
}

/// Generated mode: one to six patterns and a text, read from the bytes.
fn generated(data: &[u8]) -> (Vec<String>, String) {
    let mut u = Unstructured::new(data);
    let count = choose(&mut u, MAX_PATTERNS) + 1;
    let patterns = (0..count)
        .map(|_| {
            let mut budget = PATTERN_BUDGET;
            let mut pattern = String::new();
            if choose(&mut u, 6) == 0 {
                pattern.push_str("(?i)");
            }
            if choose(&mut u, 8) == 0 {
                pattern.push_str("(?m)");
            }
            Generator {
                u: &mut u,
                groups: 0,
            }
            .alternation(&mut pattern, 0, &mut budget);
            pattern
        })
        .collect();
    let mut text = String::new();
    let pieces = choose(&mut u, 24);
    for _ in 0..pieces {
        let piece = if choose(&mut u, 4) == 0 {
            SUBSTRINGS[choose(&mut u, SUBSTRINGS.len())]
        } else {
            ALPHABET[choose(&mut u, ALPHABET.len())]
        };
        if text.len() + piece.len() > MAX_TEXT_BYTES {
            break;
        }
        text.push_str(piece);
    }
    (patterns, text)
}

/// A number below `n` from the bytes, zero once they are used up.
fn choose(u: &mut Unstructured, n: usize) -> usize {
    debug_assert!(n > 0);
    u.int_in_range(0..=n - 1).unwrap_or(0)
}

/// The text alphabet: ASCII, newlines, the characters with special folds
/// (ß, ſ, the Kelvin sign), a Greek pair, a CJK character, an emoji and a
/// combining mark.
const ALPHABET: &[&str] = &[
    "a", "b", "c", "d", "x", "y", "z", "0", "1", "_", " ", "\n", "\r", ".", "-", ":", "(", ")",
    "\"", "/", "k", "K", "s", "S", "é", "ü", "ß", "ſ", "\u{212A}", "σ", "Σ", "日", "😀",
    "\u{0301}",
];

const SUBSTRINGS: &[&str] = &[
    "ab",
    "abab",
    "foo",
    "ba",
    "ss",
    "kelvin",
    "Kelvin",
    "street",
    "fiat",
    "xyz",
    "aa",
    "abc",
    "a.b",
    "(a)",
    "a\nb",
    "é\u{0301}",
    "ſt",
];

/// Characters a literal may hold, metacharacters escaped where needed.
const LITERALS: &[&str] = &[
    "a", "b", "c", "x", "z", "0", "_", " ", "\\.", "\\-", ":", "\\(", "\\)", "\"", "/", "\\n", "é",
    "ü", "ß", "ſ", "\u{212A}", "σ", "日", "😀", "\u{0301}", "k", "s", "S",
];

/// Words for folded literals and literal tries.
const WORDS: &[&str] = &[
    "kelvin", "street", "fiat", "xyz", "ss", "ß", "ſ", "abc", "foo", "ab", "k", "K", "σ", "ſt",
    "Kelvin",
];

const CLASS_ATOMS: &[&str] = &[
    "a",
    "b",
    "x",
    "0",
    "_",
    "-",
    ".",
    "a-c",
    "x-z",
    "0-9",
    "\\w",
    "\\W",
    "\\d",
    "\\D",
    "\\s",
    "\\S",
    "\\h",
    "\\H",
    "[:alpha:]",
    "[:^alpha:]",
    "[:digit:]",
    "[:space:]",
    "[:word:]",
    "[:punct:]",
    "\\p{L}",
    "\\p{Lu}",
    "\\P{N}",
    "\\p{Greek}",
    "\\p{Han}",
    "\\p{M}",
    "é",
    "ß",
    "ſ",
    "\u{212A}",
    "σ",
    "日",
    "😀",
    "\u{0301}",
    "\\n",
    "\\x{80}-\\x{10FFFF}",
    "\\x{0}-\\x{7F}",
];

const CTYPES: &[&str] = &["\\w", "\\W", "\\d", "\\D", "\\s", "\\S", "\\h", "\\H"];

const ANCHORS: &[&str] = &[
    "\\b", "\\B", "\\<", "\\>", "^", "$", "\\A", "\\z", "\\Z", "\\G",
];

const SPECIALS: &[&str] = &[".", "\\X", "\\R", "\\y", "\\Y", "\\K"];

const PROPERTIES: &[&str] = &[
    "\\p{L}",
    "\\p{Lu}",
    "\\P{N}",
    "\\p{Greek}",
    "\\p{Han}",
    "\\p{M}",
    "\\p{Word}",
    "\\P{Word}",
];

/// One walk over the bytes, writing patterns.
struct Generator<'a, 'b> {
    u: &'a mut Unstructured<'b>,
    /// Capture groups opened so far, for back references, calls and
    /// conditions.
    groups: u32,
}

impl Generator<'_, '_> {
    fn pick(&mut self, n: usize) -> usize {
        choose(self.u, n)
    }

    fn alternation(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        let branches = if depth < MAX_DEPTH {
            self.pick(3) + 1
        } else {
            1
        };
        for branch in 0..branches {
            if branch > 0 {
                out.push('|');
            }
            self.sequence(out, depth, budget);
        }
    }

    fn sequence(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        let items = self.pick(3) + 1;
        for _ in 0..items {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            self.atom(out, depth, budget);
            self.quantifier(out);
        }
    }

    fn quantifier(&mut self, out: &mut String) {
        match self.pick(12) {
            0 => out.push('*'),
            1 => out.push('+'),
            2 => out.push('?'),
            3 => {
                let n = self.pick(4);
                out.push_str(&format!("{{{n}}}"));
            }
            4 => {
                let n = self.pick(4);
                out.push_str(&format!("{{{n},}}"));
            }
            5 => {
                let n = self.pick(3);
                let m = n + self.pick(3);
                out.push_str(&format!("{{{n},{m}}}"));
            }
            6 => {
                let m = self.pick(4);
                out.push_str(&format!("{{,{m}}}"));
            }
            // Above the bound the seek keeps for counted repetitions.
            7 => out.push_str("{0,70}"),
            _ => return,
        }
        match self.pick(4) {
            0 => out.push('?'),
            1 => out.push('+'),
            _ => {}
        }
    }

    fn atom(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        let choices = if depth < MAX_DEPTH { 16 } else { 7 };
        match self.pick(choices) {
            0 | 1 => {
                let count = self.pick(3) + 1;
                for _ in 0..count {
                    out.push_str(LITERALS[self.pick(LITERALS.len())]);
                }
            }
            2 => self.class(out, 0),
            3 => out.push_str(CTYPES[self.pick(CTYPES.len())]),
            4 => out.push_str(ANCHORS[self.pick(ANCHORS.len())]),
            5 => out.push_str(SPECIALS[self.pick(SPECIALS.len())]),
            6 => out.push_str(PROPERTIES[self.pick(PROPERTIES.len())]),
            7 => self.group(out, depth, budget),
            8 => self.look(out, depth, budget),
            9 => self.back_reference(out),
            10 => self.call(out),
            11 => self.conditional(out, depth, budget),
            12 => {
                out.push_str("(?~");
                self.alternation(out, depth + 1, budget);
                out.push(')');
            }
            13 => self.folded_words(out),
            14 => {
                // A keyword list behind a negative look, the shape a
                // grammar's keyword rules take.
                out.push_str("(?<![-\\w])(?:");
                let words = self.pick(3) + 2;
                for word in 0..words {
                    if word > 0 {
                        out.push('|');
                    }
                    out.push_str(WORDS[self.pick(WORDS.len())]);
                }
                out.push_str(")(?![.:\\w])");
            }
            _ => {
                out.push_str("(?:");
                self.alternation(out, depth + 1, budget);
                out.push(')');
            }
        }
    }

    fn class(&mut self, out: &mut String, nesting: u32) {
        out.push('[');
        if self.pick(3) == 0 {
            out.push('^');
        }
        let members = self.pick(4) + 1;
        for _ in 0..members {
            if nesting < 2 && self.pick(6) == 0 {
                self.class(out, nesting + 1);
            } else {
                out.push_str(CLASS_ATOMS[self.pick(CLASS_ATOMS.len())]);
            }
        }
        if nesting < 2 && self.pick(5) == 0 {
            out.push_str("&&");
            self.class(out, nesting + 1);
        }
        out.push(']');
    }

    fn group(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        let opener = match self.pick(8) {
            0 | 1 => {
                self.groups += 1;
                "("
            }
            2 => {
                self.groups += 1;
                "(?<n>"
            }
            3 => "(?>",
            4 => "(?i:",
            5 => "(?m:",
            6 => "(?-i:",
            _ => "(?:",
        };
        out.push_str(opener);
        self.alternation(out, depth + 1, budget);
        out.push(')');
    }

    fn look(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        let opener = match self.pick(4) {
            0 => "(?=",
            1 => "(?!",
            2 => "(?<=",
            _ => "(?<!",
        };
        out.push_str(opener);
        self.alternation(out, depth + 1, budget);
        out.push(')');
    }

    fn back_reference(&mut self, out: &mut String) {
        if self.groups == 0 {
            out.push('a');
            return;
        }
        let group = self.pick(self.groups as usize) + 1;
        match self.pick(3) {
            0 => out.push_str(&format!("\\{group}")),
            1 => out.push_str(&format!("\\k<{group}>")),
            _ => out.push_str("\\k<n>"),
        }
    }

    fn call(&mut self, out: &mut String) {
        if self.groups == 0 {
            out.push_str("\\g<0>");
            return;
        }
        let group = self.pick(self.groups as usize) + 1;
        match self.pick(3) {
            0 => out.push_str(&format!("\\g<{group}>")),
            1 => out.push_str("\\g<n>"),
            _ => out.push_str("\\g<0>"),
        }
    }

    fn conditional(&mut self, out: &mut String, depth: u32, budget: &mut u32) {
        out.push_str("(?(");
        match self.pick(3) {
            0 if self.groups > 0 => {
                let group = self.pick(self.groups as usize) + 1;
                out.push_str(&group.to_string());
            }
            1 if self.groups > 0 => out.push_str("<n>"),
            _ => self.sequence(out, depth + 1, budget),
        }
        out.push(')');
        self.sequence(out, depth + 1, budget);
        if self.pick(2) == 0 {
            out.push('|');
            self.sequence(out, depth + 1, budget);
        }
        out.push(')');
    }

    /// A folded word, or a list of them, which the tuner turns into a
    /// folded literal trie.
    fn folded_words(&mut self, out: &mut String) {
        if self.pick(2) == 0 {
            out.push_str("(?i)");
            out.push_str(WORDS[self.pick(WORDS.len())]);
            return;
        }
        out.push_str("(?i:");
        let words = self.pick(4) + 2;
        for word in 0..words {
            if word > 0 {
                out.push('|');
            }
            out.push_str(WORDS[self.pick(WORDS.len())]);
        }
        out.push(')');
    }
}

fn bounds(found: Option<ScannerMatch>) -> Option<(usize, Vec<(usize, usize)>)> {
    found.map(|m| {
        (
            m.index,
            m.captures().iter().map(|c| (c.start, c.end)).collect(),
        )
    })
}

/// Whether the position-lead search reports the default retry limit for
/// this call: the one difference ADR-008 accepts (an attempt the pre-filter
/// leaves out can exhaust it, `(a+)+b` before `c` on a run of `a`), which
/// the scanner reports as no match. The C API's set runs the same search
/// without a pre-filter and keeps the error code.
fn retry_limit_reached(patterns: &[&str], text: &str, start: usize, bits: u32) -> bool {
    let regs = patterns
        .iter()
        .map(|pattern| {
            onig_new(
                pattern.as_bytes(),
                ONIG_OPTION_CAPTURE_GROUP,
                &ONIG_ENCODING_UTF8,
                &OnigSyntaxOniguruma,
            )
            .map(Box::new)
        })
        .collect::<Result<Vec<_>, _>>();
    let Ok(regs) = regs else {
        return false;
    };
    let (Some(mut set), _) = onig_regset_new(regs) else {
        return false;
    };
    let mut options = ONIG_OPTION_NONE;
    if bits & 1 != 0 {
        options |= ONIG_OPTION_NOT_BEGIN_STRING;
    }
    if bits & 2 != 0 {
        options |= ONIG_OPTION_NOT_END_STRING;
    }
    if bits & 4 != 0 {
        options |= ONIG_OPTION_NOT_BEGIN_POSITION;
    }
    let (index, _) = onig_regset_search(
        &mut set,
        text.as_bytes(),
        text.len(),
        start,
        text.len(),
        OnigRegSetLead::PositionLead,
        options,
    );
    index == ONIGERR_RETRY_LIMIT_IN_MATCH_OVER
}

/// The next start after a match (or a miss) at `at`: the match end, or
/// the next character after a zero-width match.
fn advance(text: &str, at: usize, found: &Option<(usize, Vec<(usize, usize)>)>) -> usize {
    let end = found.as_ref().map_or(at, |(_, captures)| captures[0].1);
    if end > at {
        return end;
    }
    let mut next = at + 1;
    while next < text.len() && !text.is_char_boundary(next) {
        next += 1;
    }
    next
}

/// Compares a scanner with the pre-filter against one without it, over
/// the patterns the scanner accepts, on `text`.
fn compare(patterns: &[&str], text: &str) {
    let plain_config = ScannerConfig::default().prefilter(false);
    let accepted: Vec<&str> = patterns
        .iter()
        .copied()
        .filter(|pattern| Scanner::with_config(&[pattern], &plain_config).is_ok())
        .collect();
    if accepted.is_empty() {
        return;
    }
    let patterns = accepted.as_slice();
    let Ok(mut plain) = Scanner::with_config(patterns, &plain_config) else {
        return;
    };
    let Ok(mut filtered) = Scanner::with_config(patterns, &ScannerConfig::default()) else {
        return;
    };
    let deadline = Instant::now() + TIME_BUDGET;

    // Every find option, from every byte offset.
    for bits in 0..8 {
        let options = ScannerFindOptions::from_bits(bits);
        for start in 0..=text.len() {
            let before = Instant::now();
            let want = bounds(plain.find_next_match(text, start, options));
            if before.elapsed() > SLOW_SEARCH {
                return;
            }
            let got = bounds(filtered.find_next_match(text, start, options));
            if got != want {
                if retry_limit_reached(patterns, text, start, bits) {
                    return;
                }
                panic!(
                    "find_next_match differs with the pre-filter: patterns {patterns:?}, text {text:?}, start {start}, options {bits}: with {got:?}, without {want:?}"
                );
            }
            if Instant::now() > deadline {
                return;
            }
        }
    }

    // The cache route: a tokenizing loop with a stable id, each call
    // repeated so the route probes its per-regex path.
    for bits in [0, 4] {
        let options = ScannerFindOptions::from_bits(bits);
        let mut at = 0;
        while at <= text.len() {
            let mut want = None;
            for _ in 0..REPEATED_CALLS {
                let before = Instant::now();
                want = bounds(plain.find_next_match_with_id(text, STRING_ID, at, options));
                if before.elapsed() > SLOW_SEARCH {
                    return;
                }
                let got = bounds(filtered.find_next_match_with_id(text, STRING_ID, at, options));
                if got != want {
                    if retry_limit_reached(patterns, text, at, bits) {
                        return;
                    }
                    panic!(
                        "find_next_match_with_id differs with the pre-filter: patterns {patterns:?}, text {text:?}, start {at}, options {bits}: with {got:?}, without {want:?}"
                    );
                }
            }
            if want.is_none() || Instant::now() > deadline {
                break;
            }
            at = advance(text, at, &want);
        }
    }

    // The UTF-16 route, from every code unit offset.
    let string = OnigString::new(text);
    for start in 0..=string.utf16_len() {
        let before = Instant::now();
        let want = bounds(plain.find_next_match_utf16(&string, start, ScannerFindOptions::NONE));
        if before.elapsed() > SLOW_SEARCH {
            return;
        }
        let got = bounds(filtered.find_next_match_utf16(&string, start, ScannerFindOptions::NONE));
        if got != want {
            let byte_start = text
                .char_indices()
                .scan(0, |units, (byte, c)| {
                    let at = *units;
                    *units += c.len_utf16();
                    Some((at, byte))
                })
                .find(|&(at, _)| at >= start)
                .map_or(text.len(), |(_, byte)| byte);
            if retry_limit_reached(patterns, text, byte_start, 0) {
                return;
            }
            panic!(
                "find_next_match_utf16 differs with the pre-filter: patterns {patterns:?}, text {text:?}, UTF-16 start {start}: with {got:?}, without {want:?}"
            );
        }
        if Instant::now() > deadline {
            return;
        }
    }
}
