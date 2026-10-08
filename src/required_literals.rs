//! Rust-only (ADR-008): literals one of which every match contains.
//!
//! A RegSet fallback entry's optimizer (C's) cannot bound where a match
//! starts. Its search checks once that the optimizer's string or byte map
//! occurs, then attempts every position up to the end of the subject. Many
//! such expressions cannot match without some literal: every successful
//! attempt from `p` contains one of a set of literals at some `q >= p`,
//! ending by the end of the subject. A search from `start` then has nothing
//! to attempt when none occurs in `start..end`, and nothing after the last
//! occurrence.
//!
//! [`derive()`] takes the optimizer's exact string as that set where the
//! optimizer has one: C's search relies on every match holding it. For a
//! byte map it reads a set from the tuned parse tree, the tree the bytecode
//! is compiled from: a C++ grammar pattern
//! `(…|\s++|(?<=\W)|(?=\W)|^|\n?$|\Z)((?<!\w)(?:static|const)_cast(?!\w))`
//! needs `_cast`, and one ending in `(?=\{)` needs `{`.
//!
//! Outside look-behind the VM does not move back past the attempt start
//! (but see look-behind leads below): a look-ahead restores the position
//! where it began, and a call runs its group from the current position. A
//! node matched at `x >= p` contributes as follows:
//!
//! - a string matches its own bytes at `x` (`tune_tree` has unraveled
//!   case-insensitive strings into classes, alternations and exact
//!   strings); a class of at most [`MAX_CLASS_BYTES`] ASCII bytes one of
//!   them; a case-sensitive literal trie one of its literals;
//! - a concatenation runs every element, so any element's set serves; the
//!   most selective one is kept ([`Set::rank`]);
//! - an alternation runs one branch: the union, if every branch has a set;
//! - a capture, option or atomic group runs its body, a quantifier with a
//!   minimum of at least one its body at least once;
//! - a positive look-ahead matches its body from `x` before it restores the
//!   position;
//! - everything else contributes nothing: look-behind (its text may lie
//!   before the attempt start), negative look-arounds, back references,
//!   calls (their group counts where it stands), conditions, other classes
//!   and types, anchors, and optional parts.
//!
//! Expressions with callouts, `\K` or absent operators get no set: they
//! observe attempts, move the match start, or move the position back to a
//! saved one. A folded (case-insensitive) literal trie gives nothing either:
//! it also matches non-ASCII input such as the Kelvin sign for `k`, which no
//! finder over its ASCII literals sees.
//!
//! Position checks rest on an invariant of an attempt from `p`: every byte
//! between the VM's position outside look-behind and `p` is a trailing
//! byte (`0x80..=0xBF`). No literal of a set starts with one, so each lies
//! at or after `p`. Moving forward keeps the invariant, and so does going
//! back: a look-ahead, a fixed-length look-behind and backtracking return
//! to where they began; a negative look-behind of variable length leaves
//! through the alternative it pushed at its position, and a positive one
//! goes on only where its body ends at its position (`CheckPosition`
//! against the right range it set there). `\G` only tests the position.
//! Without the case below the position never lies before `p`, whatever
//! strings the expression holds.
//!
//! A look-behind that checks its trailing literal first (`lead_node`,
//! `OpCode::Move`) steps back as many characters as the literal has,
//! matches its bytes forward and, if positive, goes on where they end. For
//! a string C accepts, that end keeps the invariant. Ferroni also accepts
//! `\x{140000}`, which C rejects (`USE_CHECK_VALIDITY_OF_STRING_IN_TREE`):
//! its `F5 80 80 80` counts as four characters but is passed in one step
//! back, and the match can go on before `p` with no trailing byte there.
//! The RegSet therefore keeps every attempt of an entry with such a check,
//! as of one with callouts (ADR-008); including those entries waits for
//! C's string validity check.

use std::sync::OnceLock;

use crate::regexec::MatchArg;
use crate::regint::*;
use crate::regparse_types::*;

/// Most literals a set holds; larger unions give nothing.
const MAX_LITERALS: usize = 64;

/// Most members of a class that counts as a set of one-byte literals.
const MAX_CLASS_BYTES: usize = 4;

/// Literal lengths from here on rank equally: such literals are rare
/// anyway, and fewer of them search faster.
const RANKED_LEN: usize = 4;

/// Every ASCII byte, so a class member is a literal like any other.
static ASCII_BYTES: [u8; 128] = {
    let mut bytes = [0; 128];
    let mut b = 0;
    while b < 128 {
        bytes[b] = b as u8;
        b += 1;
    }
    bytes
};

/// How common each byte is in prose and source code, higher is more
/// frequent (`crate::leading_run::COMMON_BYTES`).
static BYTE_RANKS: [u8; 256] = {
    let common = crate::leading_run::COMMON_BYTES;
    let mut ranks = [0; 256];
    let mut at = 0;
    while at < common.len() {
        ranks[common[at] as usize] = (common.len() - at) as u8;
        at += 1;
    }
    ranks
};

#[cfg(test)]
thread_local! {
    /// Derives sets regardless of the optimizer, so the analysis can be
    /// tested on expressions that never become RegSet fallback entries.
    pub(crate) static UNGATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Literals one of which every match contains, at or after the position its
/// attempt started at.
pub(crate) struct RequiredLiterals {
    /// The literals, one after the other, sorted by length; none holds
    /// another.
    bytes: Box<[u8]>,
    /// Where each literal ends in `bytes`.
    ends: Box<[u32]>,
    /// The program calls subexpressions, which a per-search limit counts.
    calls: bool,
    /// Built on first use: most compiled expressions never reach a
    /// fallback search.
    searcher: OnceLock<Searcher>,
}

enum Searcher {
    /// One to three single-byte literals (`memchr`, `memchr2`, `memchr3`).
    Bytes(usize, [u8; 3]),
    /// Boxed: the finder is far larger than the other variants.
    Literal(Box<memchr::memmem::Finder<'static>>),
    Literals(aho_corasick::AhoCorasick),
    /// The automaton could not be built (it exceeded the crate's size
    /// limits): every position stays a candidate.
    Unavailable,
}

impl RequiredLiterals {
    fn literals(&self) -> impl Iterator<Item = &[u8]> {
        let starts = std::iter::once(0).chain(self.ends.iter().copied());
        starts
            .zip(self.ends.iter().copied())
            .map(|(start, end)| &self.bytes[start as usize..end as usize])
    }

    #[cfg(test)]
    pub(crate) fn literal_list(&self) -> Vec<Vec<u8>> {
        self.literals().map(<[u8]>::to_vec).collect()
    }

    fn searcher(&self) -> Searcher {
        let count = self.ends.len();
        if count == 1 && self.bytes.len() > 1 {
            Searcher::Literal(Box::new(
                memchr::memmem::Finder::new(&self.bytes).into_owned(),
            ))
        } else if count <= 3 && self.bytes.len() == count {
            let mut bytes = [0; 3];
            bytes[..count].copy_from_slice(&self.bytes);
            Searcher::Bytes(count, bytes)
        } else {
            // Leftmost semantics report the match that starts first.
            aho_corasick::AhoCorasick::builder()
                .match_kind(aho_corasick::MatchKind::LeftmostFirst)
                .build(self.literals())
                .map_or(Searcher::Unavailable, Searcher::Literals)
        }
    }

    /// Whether a search may leave out the attempts the literals rule out.
    /// Such an attempt fails, but only after the backtracks it takes: the
    /// search retry budget, stack and time limits and a limit on calls per
    /// search could observe that, and FIND_LONGEST keeps searching. The
    /// retry limit in match is left to ADR-008's deliberate difference: a
    /// left-out attempt that would have stopped at it reports no match.
    #[inline]
    pub(crate) fn applies(&self, msa: &MatchArg, find_longest: bool) -> bool {
        msa.retry_limit_in_search == 0
            && msa.match_stack_limit == 0
            && msa.time_limit == 0
            && !find_longest
            && (!self.calls || crate::regexec::onig_get_subexp_call_limit_in_search() == 0)
    }

    /// The first position in `from..end` where one of the literals starts
    /// and ends by `end`. An attempt after the last such position cannot
    /// match.
    #[inline]
    pub(crate) fn find(&self, text: &[u8], from: usize, end: usize) -> Option<usize> {
        if from >= end {
            return None;
        }
        let hay = &text[from..end];
        let at = match self.searcher.get_or_init(|| self.searcher()) {
            Searcher::Bytes(1, [a, ..]) => memchr::memchr(*a, hay),
            Searcher::Bytes(2, [a, b, _]) => memchr::memchr2(*a, *b, hay),
            Searcher::Bytes(_, [a, b, c]) => memchr::memchr3(*a, *b, *c, hay),
            Searcher::Literal(finder) => finder.find(hay),
            Searcher::Literals(automaton) => automaton.find(hay).map(|m| m.start()),
            Searcher::Unavailable => Some(0),
        }?;
        Some(from + at)
    }
}

/// The required literals of a compiled expression, from its optimizer (C's)
/// and its tuned parse tree `root`; `env` is its parse environment. Only
/// expressions whose optimizer leaves the match start unbounded are
/// considered: the RegSet searches them on their own (fallback entries).
pub(crate) fn derive(root: &Node, reg: &RegexType, env: &ParseEnv) -> Option<RequiredLiterals> {
    #[cfg(test)]
    let ungated = UNGATED.with(|ungated| ungated.get());
    #[cfg(not(test))]
    let ungated = false;
    let exact = matches!(
        reg.optimize,
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
    ) && !reg.exact.is_empty();
    let unbounded_start =
        reg.dist_max == INFINITE_LEN && (reg.optimize == OptimizeType::Map || exact);
    // The parser numbers the gimmicks of `\K` and absent operators, and
    // nothing else (`ParseEnv::id_entry`).
    if !(ungated || unbounded_start)
        || !crate::regenc::onigenc_is_ascii_compatible_encoding(reg.enc)
        || env.id_num != 0
        || env.keep_num != 0
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
    {
        return None;
    }
    let calls = env.num_call > 0;
    let single = |literal: &[u8]| {
        Some(RequiredLiterals {
            bytes: literal.into(),
            ends: Box::new([u32::try_from(literal.len()).ok()?]),
            calls,
            searcher: OnceLock::new(),
        })
    };
    // Every match holds the optimizer's exact string at least `dist_min`
    // bytes after its start. A tree set is rarely more selective: reading
    // these trees as well saved another 0.3% of the captured C++ replay's
    // instructions for 60% more analysis time.
    if exact && !ungated {
        return single(&reg.exact);
    }
    let mut walk = Walk {
        tries: &reg.literal_tries,
        literals: Vec::new(),
    };
    let set = walk.node(root)?;
    if set.count() == 1 {
        return single(walk.literals[set.start as usize]);
    }
    let literals = &mut walk.literals[set.range()];
    literals.sort_unstable_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)));
    // A literal that holds another is redundant: each of its occurrences
    // holds that one too.
    let mut bytes = Vec::new();
    let mut ends: Vec<u32> = Vec::with_capacity(literals.len());
    for (at, literal) in literals.iter().enumerate() {
        if !literals[..at].iter().any(|shorter| {
            shorter.len() <= literal.len()
                && literal
                    .windows(shorter.len())
                    .any(|window| window == *shorter)
        }) {
            bytes.extend_from_slice(literal);
            ends.push(u32::try_from(bytes.len()).ok()?);
        }
    }
    Some(RequiredLiterals {
        bytes: bytes.into_boxed_slice(),
        ends: ends.into_boxed_slice(),
        calls,
        searcher: OnceLock::new(),
    })
}

/// How common a literal is: the rank of its rarest byte.
fn rarity(literal: &[u8]) -> u32 {
    literal
        .iter()
        .map(|&b| BYTE_RANKS[b as usize] as u32)
        .min()
        .unwrap_or(0)
}

/// A set of literals: `Walk::literals[start..end]`, never empty.
#[derive(Clone, Copy)]
struct Set {
    start: u32,
    end: std::num::NonZeroU32,
    /// The length of the shortest literal.
    min_len: u32,
    /// How common the literals are: the rank of each literal's rarest
    /// byte, summed.
    common: u32,
}

impl Set {
    fn count(&self) -> usize {
        (self.end.get() - self.start) as usize
    }

    fn range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end.get() as usize
    }

    /// Higher is more selective: longer literals (up to [`RANKED_LEN`]),
    /// then fewer of them, then rarer bytes.
    fn rank(&self) -> (usize, std::cmp::Reverse<usize>, std::cmp::Reverse<u32>) {
        (
            (self.min_len as usize).min(RANKED_LEN),
            std::cmp::Reverse(self.count()),
            std::cmp::Reverse(self.common),
        )
    }
}

/// One walk over a tuned parse tree. The literals of every set found are
/// borrowed from the tree and its tries, and kept in one buffer.
struct Walk<'a> {
    tries: &'a [crate::literal_trie::LiteralTrie],
    literals: Vec<&'a [u8]>,
}

impl<'a> Walk<'a> {
    /// A set of the literals `literals`, at most [`MAX_LITERALS`], none
    /// empty.
    fn push(&mut self, literals: impl IntoIterator<Item = &'a [u8]>) -> Option<Set> {
        let start = self.literals.len();
        let mut min_len = usize::MAX;
        let mut common = 0u32;
        for literal in literals {
            if literal.is_empty() || self.literals.len() - start == MAX_LITERALS {
                self.literals.truncate(start);
                return None;
            }
            min_len = min_len.min(literal.len());
            common += rarity(literal);
            self.literals.push(literal);
        }
        Some(Set {
            start: u32::try_from(start).ok()?,
            end: std::num::NonZeroU32::new(u32::try_from(self.literals.len()).ok()?)
                .filter(|end| end.get() as usize > start)?,
            min_len: u32::try_from(min_len).ok()?,
            common,
        })
    }

    /// The set of `node`, which every match runs at or after its attempt
    /// start. Parts that do not always run are not visited.
    fn node(&mut self, node: &'a Node) -> Option<Set> {
        if node.has_status(ND_ST_LITERAL_ALT) {
            let trie = self
                .tries
                .get(crate::regcomp::literal_alt_trie_index(node)?)?;
            if trie.is_case_insensitive() || trie.literals().len() > MAX_LITERALS {
                return None;
            }
            return self.push(trie.literals().iter().map(Vec::as_slice));
        }
        match &node.inner {
            NodeInner::String(sn) => {
                // `tune_tree` unravels every case-insensitive string but a
                // crude one, which compiles to its exact bytes.
                if node.has_status(ND_ST_IGNORECASE) && !sn.is_crude() {
                    return None;
                }
                self.push([&sn.s[..]])
            }
            NodeInner::CClass(cc) => {
                if cc.is_not() || cc.mbuf.is_some() {
                    return None;
                }
                let mut bytes: [usize; MAX_CLASS_BYTES] = [0; MAX_CLASS_BYTES];
                let mut count = 0;
                for member in bitset_members(&cc.bs) {
                    if member >= 0x80 || count == MAX_CLASS_BYTES {
                        return None;
                    }
                    bytes[count] = member;
                    count += 1;
                }
                self.push(bytes[..count].iter().map(|&b| &ASCII_BYTES[b..=b]))
            }
            // Every element runs: keep the most selective set.
            NodeInner::List(_) => {
                let mut best: Option<Set> = None;
                let mut cur = node;
                while let NodeInner::List(cons) = &cur.inner {
                    if let Some(set) = self.node(&cons.car) {
                        if best.is_none_or(|best| set.rank() > best.rank()) {
                            best = Some(set);
                        }
                    }
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                }
                best
            }
            // One branch runs: every branch needs a set.
            NodeInner::Alt(_) => {
                let mut union: Option<Set> = None;
                let mut cur = node;
                while let NodeInner::Alt(cons) = &cur.inner {
                    let set = self.node(&cons.car)?;
                    union = Some(match union {
                        None => set,
                        Some(union) => self.union(union, set)?,
                    });
                    match &cons.cdr {
                        Some(next) => cur = next,
                        None => break,
                    }
                }
                union
            }
            NodeInner::Quant(qn) if qn.lower >= 1 && qn.upper != 0 => {
                self.node(qn.body.as_deref()?)
            }
            // Captures, options and atomic groups run their body; a
            // condition runs one of two branches.
            NodeInner::Bag(bn) if bn.bag_type != BagType::IfElse => self.node(bn.body.as_deref()?),
            // A positive look-ahead matches its body from the current
            // position. Look-behind may read text before the attempt
            // start, and a negative look-around's body must fail.
            NodeInner::Anchor(an) if an.anchor_type == ANCR_PREC_READ => {
                self.node(an.body.as_deref()?)
            }
            _ => None,
        }
    }

    /// The union of two sets, as one range at the end of the buffer.
    fn union(&mut self, a: Set, b: Set) -> Option<Set> {
        let count = a.count() + b.count();
        if count > MAX_LITERALS {
            return None;
        }
        let start = if a.end.get() == b.start {
            // `b` was pushed right behind `a`: already one range.
            a.start as usize
        } else {
            let start = self.literals.len();
            self.literals.extend_from_within(a.range());
            self.literals.extend_from_within(b.range());
            start
        };
        Some(Set {
            start: u32::try_from(start).ok()?,
            end: std::num::NonZeroU32::new(u32::try_from(start + count).ok()?)?,
            min_len: a.min_len.min(b.min_len),
            common: a.common + b.common,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oniguruma::*;
    use crate::regcomp::onig_new;

    fn compile(pattern: &str, ungated: bool) -> Option<RegexType> {
        UNGATED.with(|flag| flag.set(ungated));
        let reg = onig_new(
            pattern.as_bytes(),
            ONIG_OPTION_NONE,
            &crate::encodings::utf8::ONIG_ENCODING_UTF8,
            &crate::regsyntax::OnigSyntaxOniguruma,
        );
        UNGATED.with(|flag| flag.set(false));
        reg.ok()
    }

    /// The literals the tree analysis derives, whatever the optimizer.
    fn analyzed(pattern: &str) -> Option<Vec<String>> {
        let reg = compile(pattern, true).unwrap_or_else(|| panic!("{pattern}"));
        reg.required_literals.map(|required| {
            required
                .literal_list()
                .into_iter()
                .map(|literal| String::from_utf8(literal).unwrap())
                .collect()
        })
    }

    fn check(cases: &[(&str, Option<&[&str]>)]) {
        for &(pattern, expected) in cases {
            let expected =
                expected.map(|literals| literals.iter().map(|l| l.to_string()).collect());
            assert_eq!(analyzed(pattern), expected, "{pattern}");
        }
    }

    #[test]
    fn concatenations_keep_their_most_selective_element() {
        check(&[
            ("abc", Some(&["abc"])),
            (r"\s*abc", Some(&["abc"])),
            (r"a\w+bcd", Some(&["bcd"])),
            // A literal trie gives its literals.
            (
                r"(?:alpha|beta|gamma|delta)x",
                Some(&["beta", "alpha", "delta", "gamma"]),
            ),
            // Of sets of equally long literals the smaller one wins.
            (
                r"(?:reinterpret|dynamic|static|const)_cast",
                Some(&["_cast"]),
            ),
            // Then the rarer bytes: `b` is rarer than `a`.
            ("(?i)ab", Some(&["B", "b"])),
            (r"\w+(?:ab|cd)\w+(?:xy|zq)", Some(&["xy", "zq"])),
            // Literals beyond ASCII are bytes like any other.
            ("é+x", Some(&["é"])),
            (r"\w+é", Some(&["é"])),
        ]);
    }

    #[test]
    fn alternations_need_a_set_from_every_branch() {
        check(&[
            (r"(?:foo|barbaz)\d", Some(&["foo", "barbaz"])),
            (r"(?:delete\s*\[\]|delete|new)", Some(&["new", "delete"])),
            // A literal that holds another adds nothing.
            ("(?:abcd|ab|xaby)", Some(&["ab"])),
            (r"(?:abc|\d)x?", None),
            (r"x(?:ab|cd|\w)", Some(&["x"])),
        ]);
        // At most `MAX_LITERALS` literals.
        let branches = |n: usize, tail: &str| {
            (0..n)
                .map(|i| format!("w{i:02}{tail}"))
                .collect::<Vec<_>>()
                .join("|")
        };
        for tail in ["", r"\d"] {
            let fits = analyzed(&format!("(?:{})", branches(MAX_LITERALS, tail)));
            assert_eq!(fits.map(|set| set.len()), Some(MAX_LITERALS), "{tail}");
            assert_eq!(
                analyzed(&format!("(?:{})", branches(MAX_LITERALS + 1, tail))),
                None,
                "{tail}"
            );
        }
    }

    #[test]
    fn groups_and_quantifiers_pass_their_body_when_it_always_runs() {
        check(&[
            ("(abc)+", Some(&["abc"])),
            ("(?:abc){2,}", Some(&["abc"])),
            ("(?>abc)d", Some(&["abc"])),
            ("(?i:x)abc", Some(&["abc"])),
            ("a?bc", Some(&["bc"])),
            ("(?:xyz)?b", Some(&["b"])),
            (r"(?:abc)?\w", None),
            (r"(?:abc)*\w", None),
            // A definition that only calls run.
            (r"(?<n>abc){0}\w", None),
            ("a*", None),
            ("", None),
            (r"\w+", None),
            (r"\d{3}", None),
        ]);
    }

    #[test]
    fn only_positive_look_aheads_contribute() {
        check(&[
            (r"(?=\{)", Some(&["{"])),
            (r"\w+(?=\s*\{)", Some(&["{"])),
            (r"(?<=abc)x", Some(&["x"])),
            (r"(?<=abc)\w", None),
            (r"(?<!abc)\w", None),
            (r"(?!abc)\w", None),
            (r"(?!a)(?=b)", Some(&["b"])),
            ("(?=)", None),
            (r"\b", None),
        ]);
    }

    #[test]
    fn small_ascii_classes_are_one_byte_literals() {
        check(&[
            (r"[{(]\w", Some(&["(", "{"])),
            ("[abcd]", Some(&["a", "b", "c", "d"])),
            ("[abcde]", None),
            ("[^{]", None),
            ("[éx]", None),
        ]);
    }

    /// `tune_tree` unravels a case-insensitive string into what the program
    /// matches: classes, and exact alternatives where a multi-character
    /// fold starts. Classes with non-ASCII members (Kelvin sign, long s)
    /// give nothing, nor does a folded literal trie.
    #[test]
    fn case_insensitive_literals_count_only_as_exact_alternatives() {
        let _limits = crate::regexec::shared_limits();
        check(&[
            (
                "(?i)static_cast",
                Some(&["ST", "St", "sT", "st", "ſT", "ſt", "ﬅ", "ﬆ"]),
            ),
            ("(?i)k", None),
            ("(?i)s", None),
            ("(?i)(?:alpha|beta|gamma|delta)", None),
        ]);
        // Every alternative is what the expression matches there.
        let reg = compile("(?i)st", true).unwrap();
        let required = reg.required_literals.as_deref().unwrap();
        for literal in required.literal_list() {
            let (at, _) = crate::regexec::onig_search(
                &reg,
                &literal,
                literal.len(),
                0,
                literal.len(),
                None,
                ONIG_OPTION_NONE,
            );
            assert_eq!(at, 0, "{literal:?}");
        }
    }

    #[test]
    fn constructs_that_move_the_match_start_give_nothing() {
        check(&[
            (r"abc\Kdef", None),
            ("(?~abc)def", None),
            ("(?~|abc)def", None),
            (r"(?~|abc|\d+)def", None),
            ("abc(?{x})", None),
        ]);
    }

    #[test]
    fn calls_and_back_references_count_where_their_group_stands() {
        check(&[
            (r"(?<n>abc)\g<n>", Some(&["abc"])),
            (r"(?<p>\((?:[^()]|\g<p>)*\))", Some(&[")"])),
            (r"(abc)\1", Some(&["abc"])),
            (r"(\w+)\1", None),
            (r"(a)?(?(1)bcd|efg)", None),
        ]);
        let calls = |pattern| {
            compile(pattern, true)
                .and_then(|reg| reg.required_literals)
                .map(|required| required.calls)
        };
        assert_eq!(calls(r"(?<n>abc)\g<n>"), Some(true));
        assert_eq!(calls(r"(abc)\1"), Some(false));
    }

    /// Without the test switch, only expressions whose optimizer leaves the
    /// start unbounded get a set; an exact optimizer string is that set.
    #[test]
    fn sets_follow_the_optimizer() {
        let derived = |pattern: &str| {
            let reg = compile(pattern, false).unwrap();
            reg.required_literals
                .map(|required| required.literal_list())
        };
        assert_eq!(derived(r"\s*abc"), Some(vec![b"abc".to_vec()]));
        assert_eq!(derived(r"\s*\{"), Some(vec![b"{".to_vec()]));
        assert_eq!(
            derived(r"(?:\s+|^)(?:foo|bar)_x"),
            Some(vec![b"bar".to_vec(), b"foo".to_vec()])
        );
        assert_eq!(derived("abc"), None);
        assert_eq!(derived(r"a\w{2}bc"), None);
        assert_eq!(derived(r"\s*abc\Kd"), None);
    }

    fn set(literals: &[&str]) -> RequiredLiterals {
        let mut bytes = Vec::new();
        let mut ends = Vec::new();
        for literal in literals {
            bytes.extend_from_slice(literal.as_bytes());
            ends.push(bytes.len() as u32);
        }
        RequiredLiterals {
            bytes: bytes.into(),
            ends: ends.into(),
            calls: false,
            searcher: OnceLock::new(),
        }
    }

    #[test]
    fn find_reports_the_first_start_whose_literal_ends_in_range() {
        let text = b"xx{ab(cd abcx";
        for (literals, from, end, expected) in [
            (&["{"][..], 0, text.len(), Some(2)),
            (&["(", "{"], 3, text.len(), Some(5)),
            (&["(", "{", "x"], 3, text.len(), Some(5)),
            (&["ab"], 4, text.len(), Some(9)),
            // An occurrence must end by `end`.
            (&["ab"], 4, 10, None),
            (&["abcd", "bc"], 0, text.len(), Some(10)),
            (&["cd", "ab", "x", "zz"], 0, text.len(), Some(0)),
            (&["cd", "ab", "zz", "yy"], 2, text.len(), Some(3)),
            (&["{"], text.len(), text.len(), None),
            (&["{"], 3, 2, None),
        ] {
            assert_eq!(
                set(literals).find(text, from, end),
                expected,
                "{literals:?} {from} {end}"
            );
        }
    }

    #[test]
    fn left_out_attempts_stay_unobservable_but_for_the_retry_limit_in_match() {
        let _limits = crate::regexec::exclusive_limits();
        let reg = compile("a", false).unwrap();
        let mut msa = MatchArg::new(&reg, ONIG_OPTION_NONE, None, 0);
        let required = set(&["a"]);
        let reset = |msa: &mut MatchArg| {
            msa.retry_limit_in_match = 10_000_000;
            msa.retry_limit_in_search = 0;
            msa.match_stack_limit = 0;
            msa.time_limit = 0;
        };
        reset(&mut msa);
        assert!(required.applies(&msa, false));
        assert!(!required.applies(&msa, true));
        msa.retry_limit_in_match = 1;
        assert!(required.applies(&msa, false));
        for limit in [
            |msa: &mut MatchArg| msa.retry_limit_in_search = 5,
            |msa: &mut MatchArg| msa.match_stack_limit = 5,
            |msa: &mut MatchArg| msa.time_limit = 5,
        ] {
            reset(&mut msa);
            limit(&mut msa);
            assert!(!required.applies(&msa, false));
        }
        // A limit on calls per search counts the calls of left-out attempts.
        reset(&mut msa);
        let calls = RequiredLiterals {
            calls: true,
            ..set(&["a"])
        };
        let previous = crate::regexec::onig_get_subexp_call_limit_in_search();
        crate::regexec::onig_set_subexp_call_limit_in_search(5);
        assert!(!calls.applies(&msa, false));
        assert!(required.applies(&msa, false));
        crate::regexec::onig_set_subexp_call_limit_in_search(0);
        assert!(calls.applies(&msa, false));
        crate::regexec::onig_set_subexp_call_limit_in_search(previous);
    }
}
