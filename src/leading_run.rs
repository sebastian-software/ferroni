//! Rust-only (ADR-008): search-side skips for expressions that start with a
//! character-class run.
//!
//! The forward search attempts a match at every position C's optimizer
//! admits. When the optimizer string sits at an unbounded distance from the
//! match start (`[\w.+-]+@…` selects `@` with distance `1..∞`), that is every
//! position, and each attempt inside a word rescans the rest of the word
//! before failing. The plans below only change which positions reach the VM.
//! The bytecode and C's optimizer choice are unchanged.
//!
//! - [`LeadingRun`]: the expression starts with `C+` (optionally inside
//!   capture or atomic groups). An attempt from `p` whose run ends at `q` tries
//!   the rest of the expression at end positions in `(p, q]`; an attempt from
//!   `p' ∈ (p, q)` tries a subset of them with the same text and the same
//!   state, so once `p` fails, every such `p'` fails too.
//! - [`RunLiteral`]: the run is atomic and a literal that cannot continue it
//!   follows. A match then starts in the run that ends at an occurrence of
//!   the literal, so only the first position of that run needs an attempt.
//! - [`SearchStartMap`]: the bytes a match can start with, derived from the
//!   bytecode, for positions C's optimizer does not narrow down.

use crate::oniguruma::{ONIGENC_CTYPE_WORD, OnigCodePoint};
use crate::regenc::{OnigEncoding, onigenc_is_ascii_compatible_encoding};
use crate::regint::*;

/// The class of a leading run, taken from its star instruction.
#[derive(Clone, Copy)]
enum RunClass<'a> {
    /// Single-byte class holding ASCII bytes only (`CClass*`).
    AsciiBits(&'a BitSet),
    /// `[0-9A-Za-z_]` (`WordAscii*`).
    AsciiWord,
    /// Encoding-aware `\w` (`Word*`).
    Word,
    /// Single-byte bitset plus multibyte code ranges (`CClassMix*`).
    Mix(&'a BitSet, &'a [u32]),
}

/// A leading `C+` whose failed attempts cover the rest of their run.
pub(crate) struct LeadingRun {
    /// The run's star instruction.
    star_pc: usize,
    /// Set when the run is atomic and a literal outside the class follows it.
    pub(crate) literal: Option<RunLiteral>,
}

/// The literal that directly follows an atomic leading run.
pub(crate) struct RunLiteral {
    finder: memchr::memmem::Finder<'static>,
}

/// Start bytes derived from the bytecode (`derive_start_byte_map`).
pub(crate) struct SearchStartMap {
    pub(crate) bytes: [u8; CHAR_MAP_SIZE],
    /// The backtracks an attempt at an excluded byte counts before it fails:
    /// the jumps of guarded pushes in front of the first character
    /// instruction, plus its failure. Skipping the attempt is unobservable
    /// for a per-match retry limit above this count.
    miss_retries: u64,
    /// The optimizer's windows span several positions, so the map narrows
    /// them down. Unbounded searches use the map regardless.
    pub(crate) in_windows: bool,
}

impl SearchStartMap {
    /// Whether an attempt the map excludes may be left out: no limit that
    /// counts it, and no FIND_LONGEST or match cache, can observe that.
    #[inline]
    pub(crate) fn skippable(&self, retry_limit_in_match: u64) -> bool {
        retry_limit_in_match == 0 || retry_limit_in_match > self.miss_retries
    }
}

/// Instructions that make the rest of the expression depend on where the
/// attempt started, or that let an attempt observe positions it skipped.
fn depends_on_attempt_start(opcode: OpCode) -> bool {
    use OpCode::*;
    matches!(
        opcode,
        CheckPosition
            | BackRef1
            | BackRef2
            | BackRefN
            | BackRefNIc
            | BackRefMulti
            | BackRefMultiIc
            | BackRefWithLevel
            | BackRefWithLevelIc
            | BackRefCheck
            | BackRefCheckWithLevel
            | EmptyCheckEndMemst
            | EmptyCheckEndMemstPush
            | MemEndRec
            | MemEndPushRec
            | Call
            | Return
            | CalloutContents
            | CalloutName
            | SaveVal
            | UpdateVar
    )
}

impl LeadingRun {
    fn class<'a>(&self, reg: &'a RegexType) -> RunClass<'a> {
        let op = &reg.ops[self.star_pc];
        match (op.opcode, &op.payload) {
            (OpCode::WordStar, _) => RunClass::Word,
            (OpCode::WordAsciiStar | OpCode::WordAsciiStarPeekNext, _) => RunClass::AsciiWord,
            (_, OperationPayload::CClass { bsp, .. })
            | (_, OperationPayload::CClassStarPeekNext { bsp, .. }) => RunClass::AsciiBits(bsp),
            (_, OperationPayload::CClassMix { bsp, mb }) => RunClass::Mix(bsp, mb),
            _ => unreachable!("leading run over a checked star instruction"),
        }
    }

    /// Whether the class can hold characters beyond ASCII. Such a run can
    /// step over bytes a malformed multibyte sequence covers.
    fn multibyte(&self, reg: &RegexType) -> bool {
        matches!(self.class(reg), RunClass::Word | RunClass::Mix(..))
    }
}

/// Plan the leading-run skips and the bytecode start map of a compiled
/// expression. Runs after every other bytecode pass.
pub(crate) fn plan(reg: &mut RegexType) {
    reg.leading_run = plan_leading_run(reg).map(Box::new);
    reg.search_start_map = plan_start_map(reg).map(Box::new);
}

fn plan_leading_run(reg: &RegexType) -> Option<LeadingRun> {
    if !onigenc_is_ascii_compatible_encoding(reg.enc)
        || reg.needs_capture_tracking
        || reg.keep_moves_match_start
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
        || (reg.anchor & ANCR_BEGIN_POSITION) != 0
        || reg.ops.iter().any(|op| depends_on_attempt_start(op.opcode))
    {
        return None;
    }
    let mut pc = 0;
    let mut mark = None;
    loop {
        match (reg.ops.get(pc)?.opcode, &reg.ops[pc].payload) {
            (OpCode::MemStart | OpCode::MemStartPush, _) => {}
            (OpCode::Mark, OperationPayload::Mark { id, .. }) => mark = Some((pc, *id)),
            _ => break,
        }
        pc += 1;
    }
    let head = &reg.ops[pc];
    let star = reg.ops.get(pc + 1)?;
    let same_class = match (head.opcode, star.opcode, &head.payload, &star.payload) {
        (OpCode::Word, OpCode::WordStar, ..) => true,
        (OpCode::WordAscii, OpCode::WordAsciiStar | OpCode::WordAsciiStarPeekNext, ..) => true,
        (
            OpCode::CClass,
            OpCode::CClassStar,
            OperationPayload::CClass { bsp: a, .. },
            OperationPayload::CClass { bsp: b, .. },
        )
        | (
            OpCode::CClass,
            OpCode::CClassStarPeekNext,
            OperationPayload::CClass { bsp: a, .. },
            OperationPayload::CClassStarPeekNext { bsp: b, .. },
        ) => a == b && a[128 / BITS_IN_ROOM..].iter().all(|&bits| bits == 0),
        (
            OpCode::CClassMix,
            OpCode::CClassMixStar,
            OperationPayload::CClassMix { bsp: a, mb: m },
            OperationPayload::CClassMix { bsp: b, mb: n },
        ) => a == b && m == n,
        _ => false,
    };
    if !same_class {
        return None;
    }
    let mut run = LeadingRun {
        star_pc: pc + 1,
        literal: None,
    };
    // The reverse scan needs the atomic form: every skipped attempt then
    // fails with exactly one backtrack, at the run's head or at the literal.
    // Closing captures between the cut and the literal push no alternative.
    let mut literal_pc = pc + 3;
    while reg
        .ops
        .get(literal_pc)
        .is_some_and(|op| matches!(op.opcode, OpCode::MemEnd | OpCode::MemEndPush))
    {
        literal_pc += 1;
    }
    if let (Some((mark_pc, id)), Some(cut), Some(literal)) =
        (mark, reg.ops.get(pc + 2), reg.ops.get(literal_pc))
    {
        let atomic = mark_pc + 1 == pc
            && matches!(
                cut.payload,
                OperationPayload::CutToMark {
                    id: cut_id,
                    restore_pos: false,
                } if cut_id == id
            );
        let bytes = match (literal.opcode, &literal.payload) {
            (
                OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5,
                OperationPayload::Exact { s },
            ) => Some(&s[..literal.opcode as usize - OpCode::Str1 as usize + 1]),
            (OpCode::StrN, OperationPayload::ExactN { s, n }) => Some(&s[..*n as usize]),
            _ => None,
        };
        if let Some(bytes) = bytes
            .filter(|bytes| atomic && bytes[0] < 0x80 && !ascii_member(run.class(reg), bytes[0]))
        {
            run.literal = Some(RunLiteral {
                finder: memchr::memmem::Finder::new(bytes).into_owned(),
            });
        }
    }
    Some(run)
}

fn plan_start_map(reg: &RegexType) -> Option<SearchStartMap> {
    if reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0) {
        return None;
    }
    let bytes = crate::first_bytes::derive_start_byte_map(reg)?;
    if bytes.iter().all(|&b| b != 0) {
        return None;
    }
    let miss_retries = miss_retries(reg, &bytes)?;
    Some(SearchStartMap {
        bytes,
        miss_retries,
        in_windows: reg.dist_max != 0 && reg.dist_max != INFINITE_LEN,
    })
}

/// The backtracks an attempt counts at a byte `bytes` excludes, or `None`
/// where that depends on more than the leading instructions. Captures and
/// marks push no alternative. A guarded push whose main path starts with a
/// start byte jumps at an excluded byte and counts its fixed backtracks; the
/// first character instruction then fails once.
fn miss_retries(reg: &RegexType, bytes: &[u8; CHAR_MAP_SIZE]) -> Option<u64> {
    /// Leading instructions followed before giving up.
    const MAX_STEPS: usize = 32;
    use OpCode::*;
    let starts_only =
        |bsp: &BitSet| (0..SINGLE_BYTE_SIZE).all(|b| !bitset_at(bsp, b) || bytes[b] != 0);
    let jump = |pc: usize, addr: RelAddrType| usize::try_from(pc as i64 + i64::from(addr)).ok();
    let mut pc = 0;
    let mut retries = 0;
    for _ in 0..MAX_STEPS {
        let op = reg.ops.get(pc)?;
        match (op.opcode, &op.payload) {
            (MemStart | MemStartPush | Mark, _) => pc += 1,
            (
                PushOrJumpByteSet,
                OperationPayload::PushOrJumpByteSet {
                    addr,
                    bsp,
                    skipped_retries,
                },
            ) if *skipped_retries & GUARD_RETRIES_BY_CHECKS == 0 && starts_only(bsp) => {
                retries += u64::from(*skipped_retries);
                pc = jump(pc, *addr)?;
            }
            (
                PushOrJumpExact1,
                OperationPayload::PushOrJumpExact1 {
                    addr,
                    c,
                    skipped_retries,
                },
            ) if *skipped_retries & GUARD_RETRIES_BY_CHECKS == 0 && bytes[*c as usize] != 0 => {
                retries += u64::from(*skipped_retries);
                pc = jump(pc, *addr)?;
            }
            (
                Str1 | Str2 | Str3 | Str4 | Str5 | StrN | StrMb2n1 | StrMb2n2 | StrMb2n3 | StrMb2n
                | StrMb3n | StrMbn | CClass | CClassMb | CClassMix | CClassNot | CClassMbNot
                | CClassMixNot | CClassRun | Word | WordAscii | NoWord | NoWordAscii | AnyChar
                | AnyCharMl | AltLiterals,
                _,
            ) => return Some(retries + 1),
            _ => return None,
        }
    }
    None
}

#[inline]
fn ascii_member(class: RunClass, byte: u8) -> bool {
    match class {
        RunClass::AsciiBits(bsp) | RunClass::Mix(bsp, _) => bitset_at(bsp, byte as usize),
        RunClass::AsciiWord | RunClass::Word => byte.is_ascii_alphanumeric() || byte == b'_',
    }
}

/// The end of the multibyte character at `s` if the class holds it, as the
/// star instruction decides: the same length and code point, stopping where
/// a character would cross `limit` or a lone byte stands.
fn multibyte_member(
    class: RunClass,
    enc: OnigEncoding,
    text: &[u8],
    s: usize,
    limit: usize,
) -> Option<usize> {
    let len = enc.mbc_enc_len(&text[s..]);
    let next = s
        .checked_add(len)
        .filter(|&next| len > 1 && next <= limit)?;
    let code: OnigCodePoint = enc.mbc_to_code(&text[s..], limit - s);
    let member = match class {
        RunClass::Word => enc.is_code_ctype(code, ONIGENC_CTYPE_WORD),
        RunClass::Mix(bsp, mb) => {
            crate::regexec::is_in_code_range(mb, code)
                || ((code as usize) < SINGLE_BYTE_SIZE && bitset_at(bsp, code as usize))
        }
        RunClass::AsciiBits(_) | RunClass::AsciiWord => false,
    };
    member.then_some(next)
}

/// Whether a run could still hold the character starting with `byte`. An
/// ASCII byte outside the class ends every run, so a failed attempt right
/// before it has nothing to skip.
#[inline]
pub(crate) fn may_continue(reg: &RegexType, run: &LeadingRun, byte: u8) -> bool {
    byte >= 0x80 || ascii_member(run.class(reg), byte)
}

/// The end of the run from `s`, stopping at `limit`. The run steps by the
/// encoding's character length, as the VM's star loop and the search loop
/// do, so every position it passes is a boundary for both. Where the VM
/// could treat a byte differently (a lone byte from 0x80, a character cut by
/// `limit`), the scan stops earlier, which only skips less.
pub(crate) fn run_end(
    reg: &RegexType,
    run: &LeadingRun,
    text: &[u8],
    limit: usize,
    mut s: usize,
) -> usize {
    let class = run.class(reg);
    let byte_steps = !run.multibyte(reg) || reg.enc.max_enc_len() == 1;
    while s < limit {
        let byte = text[s];
        if byte < 0x80 {
            if !ascii_member(class, byte) {
                break;
            }
            s += 1;
        } else if byte_steps {
            break;
        } else {
            match multibyte_member(class, reg.enc, text, s, limit) {
                Some(next) => s = next,
                None => break,
            }
        }
    }
    s
}

/// Where the run that ends at `k` starts, no earlier than `floor`, exactly as
/// the VM's run would reach `k`. `None` when the scan cannot decide that: a
/// malformed multibyte sequence in `floor..k` may let the VM's run step over
/// a byte the scan stops at.
pub(crate) fn run_start_before(
    reg: &RegexType,
    run: &LeadingRun,
    text: &[u8],
    floor: usize,
    k: usize,
) -> Option<usize> {
    let class = run.class(reg);
    // Valid UTF-8 also makes every character head in `floor..=k` a position
    // the search loop steps to from `floor`.
    if reg.enc.max_enc_len() > 1 && std::str::from_utf8(&text[floor..k]).is_err() {
        return None;
    }
    let mut r = k;
    while r > floor {
        let byte = text[r - 1];
        if byte < 0x80 {
            if !ascii_member(class, byte) {
                break;
            }
            r -= 1;
            continue;
        }
        if !run.multibyte(reg) {
            // An ASCII-only class never holds this byte, as for the VM.
            break;
        }
        if reg.enc.max_enc_len() == 1 {
            // The scan does not decide non-ASCII bytes of a single-byte
            // encoding, so it cannot tell where the VM's run starts.
            return None;
        }
        // `floor..k` is valid UTF-8: step back to the character head.
        let mut head = r - 1;
        while (text[head] & 0xC0) == 0x80 {
            head -= 1;
        }
        if multibyte_member(class, reg.enc, text, head, k) != Some(r) {
            break;
        }
        r = head;
    }
    Some(r)
}

impl RunLiteral {
    /// The first occurrence of the literal in `text[from..to]`.
    #[inline]
    pub(crate) fn find(&self, text: &[u8], from: usize, to: usize) -> Option<usize> {
        if from >= to {
            return None;
        }
        self.finder.find(&text[from..to]).map(|i| from + i)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oniguruma::*;
    use crate::regcomp::onig_new;
    use crate::regexec::{onig_new_match_param, onig_search_with_param};

    const UTF8: OnigEncoding = &crate::encodings::utf8::ONIG_ENCODING_UTF8;
    const ASCII: OnigEncoding = &crate::encodings::ascii::ONIG_ENCODING_ASCII;

    fn compile(pattern: &str, enc: OnigEncoding) -> Option<Box<RegexType>> {
        onig_new(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            enc,
            &crate::regsyntax::OnigSyntaxOniguruma,
        )
        .ok()
        .map(Box::new)
    }

    /// The same expression without the plans: the original search path.
    fn reference(pattern: &str, enc: OnigEncoding) -> Box<RegexType> {
        let mut reg = compile(pattern, enc).unwrap();
        reg.leading_run = None;
        reg.search_start_map = None;
        reg
    }

    type Outcome = (i32, Vec<i32>, Vec<i32>);

    fn search(
        reg: &RegexType,
        text: &[u8],
        (end, start, range): (usize, usize, usize),
        option: OnigOptionType,
        mp: &crate::regexec::OnigMatchParam,
    ) -> Outcome {
        let (result, region) = onig_search_with_param(
            reg,
            text,
            end,
            start,
            range,
            Some(OnigRegion::new()),
            option,
            mp,
        );
        let region = region.unwrap();
        (result, region.beg, region.end)
    }

    /// Every match from left to right, as an iterator would find them.
    fn scan(reg: &RegexType, text: &[u8]) -> Vec<Outcome> {
        let mp = onig_new_match_param();
        let mut out = Vec::new();
        let mut start = 0;
        while start <= text.len() {
            let found = search(
                reg,
                text,
                (text.len(), start, text.len()),
                ONIG_OPTION_NONE,
                &mp,
            );
            let (pos, _, ref ends) = found;
            out.push(found.clone());
            if pos < 0 {
                break;
            }
            let match_end = ends[0] as usize;
            start = if match_end > pos as usize {
                match_end
            } else {
                pos as usize + 1
            };
        }
        out
    }

    #[test]
    fn plans_follow_the_leading_run() {
        let plan = |pattern: &str| {
            let reg = compile(pattern, UTF8).unwrap();
            (
                reg.leading_run.is_some(),
                reg.leading_run
                    .as_ref()
                    .is_some_and(|run| run.literal.is_some()),
            )
        };
        // Run and literal.
        for pattern in [
            r"[\w.+-]+@[\w-]+\.\w+",
            r"(?>\w+)@\w++",
            r"(\d+\.\d+)",
            r"[a-z]+=\d+",
            r"\w+::\w+",
        ] {
            assert_eq!(plan(pattern), (true, true), "{pattern}");
        }
        // Run only: the literal continues the run, or no literal follows.
        for pattern in [r"\w+ing\b", r"[a-z]+ing", r"\p{L}+", r"[a-z]+\s", r"\w+"] {
            assert_eq!(plan(pattern), (true, false), "{pattern}");
        }
        // Nothing: the rest depends on where the attempt started, or the
        // expression does not start with a run.
        for pattern in [
            r"\b\w+@",
            r"(\w+)\1",
            r"(\w+)@(?(1)a|b)",
            r"\w+@\G",
            r"\w+\K@",
            r"(?<n>\w+)@\g<n>",
            r"\w+@(?{x})",
            r"\w*@",
            r"a+@",
            r"(?:\w|-)+@",
            r"\w{2,}@",
            r"\A\w+@",
        ] {
            assert_eq!(plan(pattern), (false, false), "{pattern}");
        }
    }

    #[test]
    fn start_map_counts_the_backtracks_of_a_skipped_attempt() {
        let map = |pattern: &str| compile(pattern, UTF8).unwrap().search_start_map;
        let dates = map(r"(?<d>\d{2})/(?<m>\w{3})").unwrap();
        assert!(dates.in_windows);
        assert_eq!(dates.miss_retries, 1);
        assert!(!dates.skippable(1) && dates.skippable(2) && dates.skippable(0));
        // The optimizer already checks the only position of its window.
        assert!(map(r"\d+").is_none_or(|map| !map.in_windows));
        // A guarded optional sign counts its skipped push before the digit
        // fails.
        let numbers = map(r"[-+]?\d+\.\d+").unwrap();
        assert!(!numbers.in_windows);
        assert!(numbers.miss_retries > 1);
        // A guarded alternation counts its skipped push; a check that runs
        // before the first character gives no fixed count.
        assert_eq!(map(r"(?:ab|cd)+x").unwrap().miss_retries, 2);
        assert!(map(r"\b\w+x").is_none());
    }

    /// Exhaustive ranges, options and limits against the original path.
    #[test]
    fn skips_preserve_results_and_limits() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let patterns = [
            r"[\w.+-]+@\w+",
            r"(?>\w+)@(\w?)",
            r"(\d+)\.(\d+)",
            r"[ab]+:(?:c|cc)",
            r"[ab]+:(?<!a:)(c?)",
            r"\w+ing\b",
            r"[a-z]+b(?=c)",
            r"\p{L}+é",
            r"[\p{L}a]+:",
            r"\w+:$",
            r"(?<d>\d{2})/(?<m>\w{3})",
            r"[-+]?\d+\.\d+",
        ];
        let inputs: &[&[u8]] = &[
            b"",
            b"ab:",
            b"aab!ab:cc",
            b"x.y@z w@",
            b"1.2 34.5",
            b"singing ring",
            b"abbc ab",
            b"caf\xc3\xa9 \xc3\xa9t\xc3\xa9",
            b"\xc3\xa9a:b:",
            b"a\xe2b@c",
            b"ab\xc3:",
            b"\xffab:@",
            b"12/abc 1/ab",
            b"-1.5+2.25",
        ];
        let limits = [
            (0, 0, 0),
            (1, 0, 0),
            (2, 0, 0),
            (3, 0, 0),
            (0, 2, 0),
            (0, 0, 2),
        ];
        let mut comparisons = 0;
        for enc in [UTF8, ASCII] {
            for pattern in patterns {
                let Some(optimized) = compile(pattern, enc) else {
                    continue;
                };
                let reference = reference(pattern, enc);
                for &input in inputs {
                    for end in [input.len(), input.len().saturating_sub(1)] {
                        for start in 0..=end {
                            for range in [start, (start + 2).min(end), end] {
                                for option in [
                                    ONIG_OPTION_NONE,
                                    ONIG_OPTION_FIND_LONGEST,
                                    ONIG_OPTION_FIND_NOT_EMPTY,
                                ] {
                                    for (retry, search_retry, stack) in limits {
                                        let mut mp = onig_new_match_param();
                                        mp.retry_limit_in_match = retry;
                                        mp.retry_limit_in_search = search_retry;
                                        mp.match_stack_limit = stack;
                                        let at = (end, start, range);
                                        assert_eq!(
                                            search(&optimized, input, at, option, &mp),
                                            search(&reference, input, at, option, &mp),
                                            "{} {pattern:?} {input:?} {at:?} {option:?} \
                                             retry={retry} search_retry={search_retry} \
                                             stack={stack}",
                                            enc.name()
                                        );
                                        comparisons += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(comparisons > 100_000, "{comparisons}");
    }

    /// Generated expressions and subjects, compared match by match.
    #[test]
    fn generated_expressions_match_the_original_path() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let classes = [
            r"\w",
            r"\d",
            "[a-z]",
            r"[\w.+-]",
            r"[^\s]",
            r"\p{L}",
            "[A-Za-z0-9_]",
            "[ab]",
            r"[^@\n]",
            r"[\p{L}\d]",
        ];
        let quantifiers = ["+", "*", "++", "+?", "{2,}", ""];
        let wrappers = ["{}", "({})", "(?>{})", "(?:{})", "(?<n>{})", "(({}))"];
        let literals = ["@", ".", "ing", ":", "=", "é", "b", "x@", ";", "::"];
        let tails = [
            "",
            r"\w+",
            r"\d{2}",
            "(?=x)",
            r"\b",
            r"(?!\d)",
            "[a-z]*z",
            ".*?;",
            "(a|b)+",
            r"\w*$",
            "é?",
            r"(?<=@)\w",
            r"\1",
            r"(?<!b)\d",
            "(?i:X)",
            r"\p{L}+",
        ];
        let pieces: &[&[u8]] = &[
            b"a",
            b"b",
            b"@",
            b".",
            b"i",
            b"n",
            b"g",
            b"ing",
            b":",
            b"=",
            b"1",
            b"2",
            b" ",
            b"Z",
            b"x",
            b";",
            b"\n",
            b"_",
            b"-",
            b"+",
            "é".as_bytes(),
            "ü".as_bytes(),
            b"\xc3",
            b"\xff",
            b"\xe2\x82",
        ];
        let mut checked = 0;
        for _ in 0..4000 {
            let run = format!(
                "{}{}",
                classes[next(classes.len())],
                quantifiers[next(quantifiers.len())]
            );
            let pattern = format!(
                "{}{}{}",
                wrappers[next(wrappers.len())].replace("{}", &run),
                literals[next(literals.len())],
                tails[next(tails.len())]
            );
            let text: Vec<u8> = (0..next(60))
                .flat_map(|_| pieces[next(pieces.len())].to_vec())
                .collect();
            let Some(optimized) = compile(&pattern, UTF8) else {
                continue;
            };
            let reference = reference(&pattern, UTF8);
            assert_eq!(
                scan(&optimized, &text),
                scan(&reference, &text),
                "{pattern:?} {text:?}"
            );
            checked += 1;
        }
        assert!(checked > 3000, "{checked}");
    }
}
