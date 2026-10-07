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
//! - [`LeadingRun`]: the expression starts with `C+` or `C*` (optionally
//!   inside capture or atomic groups, or after `\b` for a class of word
//!   characters). An attempt from `p` whose run ends at `q` tries the rest of
//!   the expression at end positions in `[p, q]`; an attempt from `p' ∈ (p, q)`
//!   tries a subset of them with the same text and the same state, so once `p`
//!   fails, every such `p'` fails too.
//! - [`RunLiteral`]: the run is atomic and a literal that cannot continue it
//!   follows. A match then starts in the run that ends at an occurrence of
//!   the literal, so only the first position of that run needs an attempt.
//! - [`SearchStartMap`]: the bytes a match can start with, derived from the
//!   bytecode, for positions C's optimizer does not narrow down.
//! - [`SearchJump`]: the expression starts with `^`, with byte classes or
//!   literals (such as the case pairs of `(?i)regular`), or with a literal
//!   alternation. The search moves its start to the next position that can
//!   pass these checks, found with `memchr` or Aho-Corasick.

use crate::oniguruma::{ONIGENC_CTYPE_WORD, OnigCodePoint};
use crate::regenc::{OnigEncoding, onigenc_is_ascii_compatible_encoding};
use crate::regint::*;
use crate::regparse_types::*;

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

/// A leading `C+` or `C*` whose failed attempts cover the rest of their run.
pub(crate) struct LeadingRun {
    /// The run's star instruction.
    star_pc: usize,
    /// `C*`: the run may be empty, so an attempt can also start where the run
    /// ends.
    pub(crate) min_zero: bool,
    /// A word boundary `\b` precedes the run, whose class holds only word
    /// characters: no start inside a run passes it. The forward skip then only
    /// crosses ASCII bytes, where the boundary check sees the same characters
    /// as the scan.
    ascii_only: bool,
    /// An optional word before the run is redundant over ASCII starts.
    pub(crate) optional_word_prefix: bool,
    /// The run belongs to the sole positive assertion.
    pub(crate) whole_lookahead: bool,
    /// Set when the run is atomic and a literal outside the class follows it.
    pub(crate) literal: Option<RunLiteral>,
    /// The ASCII bytes of a first character narrower than the run's class
    /// (`[A-Z_a-z]\w*`). Only an attempt that read its first character
    /// stands for the later starts in its run; one that failed there says
    /// nothing about them.
    head: Option<BitSet>,
}

/// The literal that directly follows an atomic leading run.
pub(crate) struct RunLiteral {
    finder: memchr::memmem::Finder<'static>,
}

/// Rust-only (ADR-008): the expression begins with zero-width checks and
/// class repetitions only, then a literal outside those classes
/// (`(?<=[(,]|^return)\s*(\{)`). A match from `x` reads class characters up
/// to the first character outside them, where the literal has to be; so only
/// the starts in the class run that ends at an occurrence can match.
pub(crate) struct LiteralPrefix {
    finder: memchr::memmem::Finder<'static>,
    /// The ASCII bytes the leading repetitions read.
    class: BitSet,
    /// Whether they also read non-ASCII characters.
    multibyte: bool,
    /// An upper bound of the backtracks of an attempt before the literal,
    /// where the tree gives one: a per-match retry limit above it cannot
    /// observe a left-out attempt.
    retry_bound: Option<u64>,
}

impl LiteralPrefix {
    /// Whether a per-match retry limit lets the search leave attempts out.
    #[inline]
    pub(crate) fn skippable(&self, retry_limit_in_match: u64) -> bool {
        retry_limit_in_match == 0
            || self
                .retry_bound
                .is_some_and(|bound| retry_limit_in_match > bound)
    }

    /// The first occurrence of the literal in `text[from..to]`.
    #[inline]
    pub(crate) fn find(&self, text: &[u8], from: usize, to: usize) -> Option<usize> {
        if from >= to {
            return None;
        }
        self.finder.find(&text[from..to]).map(|i| from + i)
    }

    /// The starts from `s` that can reach an occurrence, as `(first, k)`:
    /// the class run before the next occurrence `k`. Only starts up to
    /// `last` matter, so the occurrence is looked for a short way past it.
    /// Where none starts there, a start up to `last` needs a class run that
    /// reaches past that window; without one nothing up to `last` can match
    /// (`None`). With one, or over malformed UTF-8, every start from `s`
    /// stays (`(s, usize::MAX)`).
    pub(crate) fn window(
        &self,
        enc: OnigEncoding,
        text: &[u8],
        s: usize,
        last: usize,
        end: usize,
    ) -> Option<(usize, usize)> {
        const REACH: usize = 64;
        let limit = end.min(last.saturating_add(REACH));
        let well_formed =
            |to: usize| enc.max_enc_len() == 1 || std::str::from_utf8(&text[s..to]).is_ok();
        // Occurrences that start before `limit`.
        let to = end.min(limit.saturating_add(self.finder.needle().len() - 1));
        match self.find(text, s, to) {
            Some(k) if well_formed(k) => Some((self.run_start(text, s, k), k)),
            Some(_) => Some((s, usize::MAX)),
            None if to >= end => None,
            None if well_formed(limit) && self.run_start(text, s, limit) > last => None,
            None => Some((s, usize::MAX)),
        }
    }

    /// Where the class run that ends at `k` starts, no earlier than `floor`:
    /// the first start that can reach the occurrence. A non-ASCII byte the
    /// classes may hold keeps every start from `floor`.
    pub(crate) fn run_start(&self, text: &[u8], floor: usize, k: usize) -> usize {
        let mut r = k;
        while r > floor {
            let byte = text[r - 1];
            if byte >= 0x80 {
                return if self.multibyte { floor } else { r };
            }
            if !bitset_at(&self.class, byte as usize) {
                break;
            }
            r -= 1;
        }
        r
    }
}

/// Plan [`LiteralPrefix`] from the tuned tree, which is gone after
/// compilation.
pub(crate) fn plan_literal_prefix(root: &Node, reg: &RegexType) -> Option<LiteralPrefix> {
    // A leading-check jump (`^`, a look-behind at a literal) already lands
    // on fewer starts than the class run before each occurrence.
    if !onigenc_is_ascii_compatible_encoding(reg.enc)
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
        || reg.search_jump.is_some()
    {
        return None;
    }
    let mut class = [0; BITSET_REAL_SIZE];
    let mut multibyte = false;
    let mut bound = Some(RetryBound { paths: 1, cost: 0 });
    let literal = match prefix_walk(root, reg.enc, &mut class, &mut multibyte, &mut bound)? {
        Prefix::Literal(bytes) => bytes,
        Prefix::Open => return None,
    };
    if literal[0] >= 0x80 || bitset_at(&class, literal[0] as usize) {
        return None;
    }
    Some(LiteralPrefix {
        finder: memchr::memmem::Finder::new(&literal).into_owned(),
        class,
        multibyte,
        // The literal's own failure, and the pops back to the bottom.
        retry_bound: bound.and_then(|b| b.paths.checked_mul(b.cost.checked_add(2)?)),
    })
}

/// What a walk over the start of the tree found.
enum Prefix {
    /// Only zero-width nodes and class repetitions so far.
    Open,
    /// They end at this case-sensitive literal.
    Literal(Vec<u8>),
}

/// `None` where a node other than a zero-width check or a class repetition
/// comes before the literal.
fn prefix_walk(
    node: &Node,
    enc: OnigEncoding,
    class: &mut BitSet,
    multibyte: &mut bool,
    bound: &mut Option<RetryBound>,
) -> Option<Prefix> {
    match &node.inner {
        // Zero-width, whatever its body reads. A look-around is atomic:
        // each of its paths fails at most once.
        NodeInner::Anchor(anchor) => {
            let cost = match anchor.body.as_deref() {
                None => Some(1),
                Some(body) => paths(body).and_then(|p| p.checked_mul(2)?.checked_add(2)),
            };
            add_cost(bound, cost);
            Some(Prefix::Open)
        }
        NodeInner::List(_) => {
            let mut cur = node;
            while let NodeInner::List(cons) = &cur.inner {
                if let Prefix::Literal(bytes) =
                    prefix_walk(&cons.car, enc, class, multibyte, bound)?
                {
                    return Some(Prefix::Literal(bytes));
                }
                match cons.cdr.as_deref() {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            Some(Prefix::Open)
        }
        // Branches that all read nothing, such as `(?:^|(?<=x))`. A later
        // failure backtracks into the next branch, so every branch is a path
        // through the rest.
        NodeInner::Alt(_) => {
            let mut cur = node;
            let mut branches = Some(RetryBound { paths: 0, cost: 0 });
            while let NodeInner::Alt(cons) = &cur.inner {
                let mut none = [0; BITSET_REAL_SIZE];
                let mut none_multibyte = false;
                let mut branch = Some(RetryBound { paths: 1, cost: 0 });
                if !matches!(
                    prefix_walk(&cons.car, enc, &mut none, &mut none_multibyte, &mut branch)?,
                    Prefix::Open
                ) || none != [0; BITSET_REAL_SIZE]
                    || none_multibyte
                {
                    return None;
                }
                branches = branches.zip(branch).and_then(|(all, one)| {
                    Some(RetryBound {
                        paths: all.paths.checked_add(one.paths)?,
                        cost: all.cost.checked_add(one.cost)?,
                    })
                });
                match cons.cdr.as_deref() {
                    Some(next) => cur = next,
                    None => break,
                }
            }
            *bound = bound.zip(branches).and_then(|(b, alt)| {
                Some(RetryBound {
                    paths: b.paths.checked_mul(alt.paths)?,
                    cost: b.cost.checked_add(alt.cost)?,
                })
            });
            Some(Prefix::Open)
        }
        // An atomic class repetition fails once and gives nothing back.
        NodeInner::Bag(bag)
            if bag.bag_type == BagType::StopBacktrack
                && matches!(
                    bag.body.as_deref().map(|b| &b.inner),
                    Some(NodeInner::Quant(_))
                ) =>
        {
            let NodeInner::Quant(quant) = &bag.body.as_deref()?.inner else {
                return None;
            };
            add_char_class(quant.body.as_deref()?, enc, class, multibyte)?;
            add_cost(bound, Some(1));
            Some(Prefix::Open)
        }
        NodeInner::Bag(bag) => match bag.bag_type {
            BagType::Memory | BagType::Option | BagType::StopBacktrack => {
                prefix_walk(bag.body.as_deref()?, enc, class, multibyte, bound)
            }
            BagType::IfElse => None,
        },
        // A repetition the rest can backtrack into: one path per count, so
        // an unbounded one leaves no bound.
        NodeInner::Quant(quant) => {
            add_char_class(quant.body.as_deref()?, enc, class, multibyte)?;
            let counts = (!is_infinite_repeat(quant.upper) && quant.upper >= quant.lower)
                .then(|| (quant.upper - quant.lower + 1) as u64);
            *bound = bound.zip(counts).and_then(|(b, n)| {
                Some(RetryBound {
                    paths: b.paths.checked_mul(n)?,
                    cost: b.cost.checked_add(n)?,
                })
            });
            Some(Prefix::Open)
        }
        NodeInner::CClass(_) | NodeInner::CType(_) => {
            add_char_class(node, enc, class, multibyte)?;
            add_cost(bound, Some(1));
            Some(Prefix::Open)
        }
        NodeInner::String(sn)
            if !sn.s.is_empty()
                && !node.has_status(ND_ST_IGNORECASE)
                && !node.has_status(ND_ST_LITERAL_ALT) =>
        {
            Some(Prefix::Literal(sn.s.clone()))
        }
        _ => None,
    }
}

/// The paths and backtracks per path of the part before the literal.
#[derive(Clone, Copy)]
struct RetryBound {
    paths: u64,
    cost: u64,
}

fn add_cost(bound: &mut Option<RetryBound>, cost: Option<u64>) {
    *bound = bound.zip(cost).and_then(|(b, c)| {
        Some(RetryBound {
            paths: b.paths,
            cost: b.cost.checked_add(c)?,
        })
    });
}

/// How many ways a look-around body can be read, where that is small and
/// known; each fails at most once.
fn paths(node: &Node) -> Option<u64> {
    const MAX_PATHS: u64 = 1 << 20;
    let p = match &node.inner {
        NodeInner::String(_) | NodeInner::CClass(_) | NodeInner::CType(_) => 1,
        NodeInner::Anchor(anchor) => match anchor.body.as_deref() {
            None => 1,
            Some(body) => paths(body)?.checked_add(1)?,
        },
        NodeInner::List(_) | NodeInner::Alt(_) => {
            let is_list = matches!(node.inner, NodeInner::List(_));
            let mut total = if is_list { 1u64 } else { 0 };
            let mut cur = Some(node);
            while let Some(NodeInner::List(cons) | NodeInner::Alt(cons)) = cur.map(|n| &n.inner) {
                let p = paths(&cons.car)?;
                total = if is_list {
                    total.checked_mul(p)?
                } else {
                    total.checked_add(p)?
                };
                cur = cons.cdr.as_deref();
            }
            total
        }
        NodeInner::Bag(bag) if bag.bag_type != BagType::IfElse => paths(bag.body.as_deref()?)?,
        NodeInner::Quant(quant) if !is_infinite_repeat(quant.upper) && quant.upper <= 8 => {
            let body = paths(quant.body.as_deref()?)?;
            let mut total = 0u64;
            let mut power = 1u64;
            for count in 0..=quant.upper {
                if count >= quant.lower {
                    total = total.checked_add(power)?;
                }
                power = power.checked_mul(body)?;
            }
            total
        }
        _ => return None,
    };
    (p <= MAX_PATHS).then_some(p)
}

/// Add the characters a class node reads to `class` (ASCII) and
/// `multibyte`; `None` for any other node.
fn add_char_class(
    node: &Node,
    enc: OnigEncoding,
    class: &mut BitSet,
    multibyte: &mut bool,
) -> Option<()> {
    match &node.inner {
        NodeInner::CClass(cc) => {
            let not = cc.is_not();
            for byte in 0..128 {
                if bitset_at(&cc.bs, byte) != not {
                    bitset_set_bit(class, byte);
                }
            }
            *multibyte |= not
                || cc.mbuf.is_some()
                || cc.bs[128 / BITS_IN_ROOM..].iter().any(|&bits| bits != 0);
        }
        NodeInner::CType(ct) if ct.ctype != CTYPE_ANYCHAR => {
            for byte in 0..128u32 {
                if enc.is_code_ctype(byte, ct.ctype as u32) != ct.not {
                    bitset_set_bit(class, byte as usize);
                }
            }
            *multibyte |= ct.not || !ct.ascii_mode;
        }
        _ => return None,
    }
    Some(())
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
    /// counts it, and no FIND_LONGEST, can observe that.
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
    reg.search_jump = plan_jump(reg).map(Box::new);
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
    let mut mark_count = 0;
    let mut boundary = None;
    let mut whole_lookahead = false;
    loop {
        match (reg.ops.get(pc)?.opcode, &reg.ops[pc].payload) {
            (OpCode::MemStart | OpCode::MemStartPush, _) => {}
            (OpCode::Mark, OperationPayload::Mark { id, .. }) => {
                // At most eight bounded scans of the compiled instructions.
                mark_count += 1;
                if mark_count > 8 {
                    return None;
                }
                let restores_position = reg.ops[pc + 1..].iter().find_map(|op| {
                    if let OperationPayload::CutToMark {
                        id: cut_id,
                        restore_pos,
                    } = &op.payload
                    {
                        (cut_id == id).then_some(*restore_pos)
                    } else {
                        None
                    }
                })?;
                // A restored start before a consuming suffix makes that
                // suffix depend on the position we would skip. A complete
                // positive assertion has no such suffix.
                if restores_position {
                    whole_lookahead = pc == 0
                        && matches!(reg.ops.last()?.opcode, OpCode::End)
                        && matches!(
                            &reg.ops.get(reg.ops.len().checked_sub(2)?)?.payload,
                            OperationPayload::CutToMark { id: cut_id, restore_pos: true }
                                if cut_id == id
                        );
                    if !whole_lookahead {
                        return None;
                    }
                }
                mark = Some((pc, *id));
            }
            (OpCode::WordBoundary, OperationPayload::WordBoundary { mode })
                if boundary.is_none() =>
            {
                boundary = Some(*mode)
            }
            _ => break,
        }
        pc += 1;
    }
    // A whole positive assertion may begin with `\w?C*`. Over ASCII,
    // the optional word cannot extend a run containing every word byte.
    // Keep the program and its optimizer intact; only plan failed starts.
    let optional_word_prefix = matches!(
        (&reg.ops.get(pc)?.opcode, &reg.ops[pc].payload),
        (OpCode::Push, OperationPayload::Push { addr: 2 })
    ) && reg
        .ops
        .get(pc + 1)
        .is_some_and(|op| matches!(op.opcode, OpCode::Word | OpCode::WordAscii));
    if optional_word_prefix {
        if !whole_lookahead || mark_count != 1 {
            return None;
        }
        pc += 2;
    }
    let head = &reg.ops[pc];
    let is_star = |op: &Operation| {
        matches!(
            op.opcode,
            OpCode::WordStar
                | OpCode::WordAsciiStar
                | OpCode::WordAsciiStarPeekNext
                | OpCode::CClassStar
                | OpCode::CClassPossessiveStar
                | OpCode::CClassStarPeekNext
                | OpCode::CClassMixStar
        )
    };
    // `C*` compiles to the star instruction alone.
    let min_zero = is_star(head);
    if min_zero && (boundary.is_some() || !byte_class_is_ascii(head)) {
        return None;
    }
    let star_pc = if min_zero { pc } else { pc + 1 };
    let star = reg.ops.get(star_pc)?;
    let same_class = min_zero
        || match (head.opcode, star.opcode, &head.payload, &star.payload) {
            (OpCode::Word, OpCode::WordStar, ..) => true,
            (OpCode::WordAscii, OpCode::WordAsciiStar | OpCode::WordAsciiStarPeekNext, ..) => true,
            (
                OpCode::CClass,
                OpCode::CClassStar | OpCode::CClassPossessiveStar,
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
    // A first character narrower than the run's class (`[A-Z_a-z]\w*`,
    // `[_a-z\x7F-ÿ][0-9_a-z\x7F-ÿ]*`): a later start in the run either fails
    // at it or reads the same run to the same end.
    let narrower_head = if same_class {
        None
    } else {
        narrower_head(reg, head, star)
    };
    if !same_class && narrower_head.is_none() {
        return None;
    }
    let mut run = LeadingRun {
        star_pc,
        min_zero,
        ascii_only: boundary.is_some() || optional_word_prefix,
        optional_word_prefix,
        whole_lookahead,
        literal: None,
        head: narrower_head,
    };
    if optional_word_prefix
        && (!min_zero
            || !(0u8..128).all(|byte| {
                !(byte.is_ascii_alphanumeric() || byte == b'_')
                    || ascii_member(run.class(reg), byte)
            }))
    {
        return None;
    }
    // A start inside a run must fail the boundary check: every member is a
    // word character in the boundary's own sense.
    if let Some(mode) = boundary {
        let word_only = match run.class(reg) {
            RunClass::AsciiWord => true,
            RunClass::Word => mode == 0,
            RunClass::AsciiBits(bsp) => (0..SINGLE_BYTE_SIZE).all(|b| {
                !bitset_at(bsp, b) || (b as u8).is_ascii_alphanumeric() || b == b'_' as usize
            }),
            RunClass::Mix(..) => false,
        };
        if !word_only {
            return None;
        }
    }
    // The reverse scan needs the atomic form: every skipped attempt then
    // fails with exactly one backtrack, at the run's head or at the literal.
    // Closing captures between the cut and the literal push no alternative.
    let cut_pc = star_pc + 1;
    let mut literal_pc = cut_pc + 1;
    while reg
        .ops
        .get(literal_pc)
        .is_some_and(|op| matches!(op.opcode, OpCode::MemEnd | OpCode::MemEndPush))
    {
        literal_pc += 1;
    }
    if let (Some((mark_pc, id)), Some(cut), Some(literal)) =
        (mark, reg.ops.get(cut_pc), reg.ops.get(literal_pc))
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
        // The reverse scan attempts a run's first position for all of it,
        // which a narrower first character may not admit.
        if let Some(bytes) = bytes.filter(|bytes| {
            atomic
                && run.head.is_none()
                && bytes[0] < 0x80
                && !ascii_member(run.class(reg), bytes[0])
        }) {
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

/// Whether a star instruction's class holds only ASCII bytes, or is `\w` or
/// a mixed class whose ASCII part the scan reads from its bitset.
/// The bitset of a first character whose class lies within the run's
/// class, both its single bytes and its multibyte ranges; `None` otherwise.
fn narrower_head(reg: &RegexType, head: &Operation, star: &Operation) -> Option<BitSet> {
    let (head_bsp, head_mb): (&BitSet, &[u32]) = match (head.opcode, &head.payload) {
        (OpCode::CClass, OperationPayload::CClass { bsp, .. }) => (bsp, &[]),
        (OpCode::CClassMix, OperationPayload::CClassMix { bsp, mb }) => (bsp, mb),
        _ => return None,
    };
    let run_class = match (star.opcode, &star.payload) {
        (OpCode::WordStar, _) => RunClass::Word,
        (OpCode::WordAsciiStar | OpCode::WordAsciiStarPeekNext, _) => RunClass::AsciiWord,
        (
            OpCode::CClassStar | OpCode::CClassPossessiveStar,
            OperationPayload::CClass { bsp, .. },
        )
        | (OpCode::CClassStarPeekNext, OperationPayload::CClassStarPeekNext { bsp, .. }) => {
            RunClass::AsciiBits(bsp)
        }
        (OpCode::CClassMixStar, OperationPayload::CClassMix { bsp, mb }) => RunClass::Mix(bsp, mb),
        _ => return None,
    };
    let ascii_within =
        (0u8..128).all(|byte| !bitset_at(head_bsp, byte as usize) || ascii_member(run_class, byte));
    // Bits from 0x80 on stand for single bytes the encoding reads alone.
    let high_within = (128..SINGLE_BYTE_SIZE).all(|byte| {
        !bitset_at(head_bsp, byte)
            || matches!(run_class, RunClass::AsciiBits(bsp) | RunClass::Mix(bsp, _) if bitset_at(bsp, byte))
    });
    // A compiled range list (`[n, from, to, ...]`) as its sorted pairs.
    fn pairs(mb: &[u32]) -> &[[u32; 2]] {
        let n = mb.first().map_or(0, |&n| n as usize);
        mb.get(1..1 + 2 * n)
            .map_or(&[][..], |flat| flat.as_chunks::<2>().0)
    }
    // Whether one of the sorted ranges holds all of `from..=to`. Adjacent
    // ranges that together hold it count as not holding it, which only plans
    // less.
    let covered = |ranges: &[[u32; 2]], from: u32, to: u32| {
        let at = ranges.partition_point(|&[_, end]| end < from);
        ranges
            .get(at)
            .is_some_and(|&[start, end]| start <= from && to <= end)
    };
    let multibyte_within = pairs(head_mb).iter().all(|&[from, to]| match run_class {
        RunClass::Mix(_, mb) => covered(pairs(mb), from, to),
        RunClass::Word => reg
            .enc
            .get_ctype_code_range(ONIGENC_CTYPE_WORD, &mut 0)
            .is_some_and(|word| covered(word.as_chunks::<2>().0, from, to)),
        RunClass::AsciiBits(_) | RunClass::AsciiWord => false,
    });
    (ascii_within && high_within && multibyte_within).then_some(*head_bsp)
}

fn byte_class_is_ascii(op: &Operation) -> bool {
    match (op.opcode, &op.payload) {
        (
            OpCode::CClassStar | OpCode::CClassPossessiveStar,
            OperationPayload::CClass { bsp, .. },
        )
        | (OpCode::CClassStarPeekNext, OperationPayload::CClassStarPeekNext { bsp, .. }) => {
            bsp[128 / BITS_IN_ROOM..].iter().all(|&bits| bits == 0)
        }
        (
            OpCode::WordStar
            | OpCode::WordAsciiStar
            | OpCode::WordAsciiStarPeekNext
            | OpCode::CClassMixStar,
            _,
        ) => true,
        _ => false,
    }
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
        RunClass::Mix(_, mb) => crate::regexec::is_in_code_range(mb, code),
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

/// Whether an attempt that failed at `byte` read the run's first character,
/// so its failure stands for the later starts in its run.
#[inline]
pub(crate) fn read_head(run: &LeadingRun, byte: u8) -> bool {
    run.head
        .as_ref()
        .is_none_or(|head| bitset_at(head, byte as usize))
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
    let byte_steps = !run.multibyte(reg) || reg.enc.max_enc_len() == 1 || run.ascii_only;
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

/// Rust-only (ADR-008): where an attempt can pass the expression's leading
/// checks, so that the search moves its start there instead of attempting
/// every position in between. Each position it passes over would fail the
/// first check it reaches, with one backtrack.
pub(crate) struct SearchJump {
    /// The expression starts with `^`: only line starts can match.
    line_start: bool,
    /// Byte classes the leading characters must belong to, such as the case
    /// pairs of `(?i)regular` (at most [`MAX_PREFIX`] of them).
    prefix: Vec<BitSet>,
    /// The class found with `memchr`: the one least likely to occur.
    probe: usize,
    probe_bytes: Vec<u8>,
    /// A leading literal alternation (`AltLiterals`), found with
    /// Aho-Corasick.
    alternation: Option<Alternation>,
    /// A leading positive look-behind at an ASCII literal, such as
    /// `(?<=took=)`: attempts start right behind its occurrences.
    behind: Option<memchr::memmem::Finder<'static>>,
}

/// The literals of a trie, searched with an Aho-Corasick automaton that is
/// built on first use: grammars compile many alternations that are never
/// searched this way (the scanner runs them through its RegSet), and the
/// automaton costs more to build than the trie.
///
/// A folded (case-insensitive) trie also matches non-ASCII input, such as
/// `K` (Kelvin sign) for `k` or `ß` for `ss`, which an automaton over ASCII
/// case variants misses. The walk reads ASCII input one literal byte at a
/// time and stops at a non-ASCII byte it cannot read, so such a match
/// starts at most `reach` bytes before the first non-ASCII byte it holds,
/// and the walk reads that byte. Those positions stay candidates.
struct Alternation {
    trie_idx: usize,
    /// The longest literal minus one byte, for a folded trie.
    reach: Option<usize>,
    automaton: std::sync::OnceLock<Option<aho_corasick::AhoCorasick>>,
}

/// The first window a folded alternation scans; each further one doubles.
const FOLDED_WINDOW: usize = 256;

/// Most prefixes a folded alternation's automaton holds.
const MAX_CASE_VARIANTS: usize = 64;

/// The ASCII case variants of the longest literal prefixes that stay within
/// [`MAX_CASE_VARIANTS`] (at least one byte). Every ASCII case variant of a
/// literal starts with one of them. An automaton over exact bytes can use
/// its vectorized prefilter, which ASCII case folding disables.
fn case_variants(literals: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let shortest = literals.iter().map(Vec::len).min().unwrap_or(0);
    let variants = |len: usize| {
        let mut out: Vec<Vec<u8>> = Vec::new();
        for literal in literals {
            let mut prefixes = vec![Vec::new()];
            for &byte in &literal[..len] {
                let cases = [byte.to_ascii_lowercase(), byte.to_ascii_uppercase()];
                let cases = if byte.is_ascii_alphabetic() {
                    &cases[..]
                } else {
                    &cases[..1]
                };
                prefixes = prefixes
                    .iter()
                    .flat_map(|prefix| {
                        cases
                            .iter()
                            .map(move |&case| [&prefix[..], &[case]].concat())
                    })
                    .collect();
            }
            out.extend(prefixes);
        }
        out.sort_unstable();
        out.dedup();
        out
    };
    let mut best = variants(1.min(shortest));
    for len in 2..=shortest {
        let more = variants(len);
        if more.len() > MAX_CASE_VARIANTS {
            break;
        }
        best = more;
    }
    best
}

impl Alternation {
    /// The first position in `s..limit` where a literal can start, or `s`
    /// without an automaton. `None` when no literal starts there.
    fn candidate(&self, reg: &RegexType, text: &[u8], s: usize, limit: usize) -> Option<usize> {
        let trie = &reg.literal_tries[self.trie_idx];
        let automaton = self.automaton.get_or_init(|| {
            let mut builder = aho_corasick::AhoCorasick::builder();
            builder.match_kind(aho_corasick::MatchKind::LeftmostFirst);
            if trie.is_case_insensitive() {
                builder.build(case_variants(trie.literals())).ok()
            } else {
                builder.build(trie.literals()).ok()
            }
        });
        // Without an automaton every position stays a candidate.
        let Some(automaton) = automaton else {
            return Some(s);
        };
        match self.reach {
            None => Some(s + automaton.find(&text[s..limit])?.start()),
            Some(reach) => Self::folded_candidate(trie, automaton, reach, text, s, limit),
        }
    }

    /// [`Alternation::candidate`] for a folded trie.
    #[inline(never)]
    fn folded_candidate(
        trie: &crate::literal_trie::LiteralTrie,
        automaton: &aho_corasick::AhoCorasick,
        reach: usize,
        text: &[u8],
        s: usize,
        limit: usize,
    ) -> Option<usize> {
        // Windows of growing size keep the scans proportional to the
        // distance moved: a search restarts behind every failed candidate.
        let mut from = s;
        let mut window = FOLDED_WINDOW;
        while from < limit {
            // Candidates in `from..to`. A literal or a folded match starting
            // there ends by `to + reach`.
            let to = limit.min(from.saturating_add(window));
            let hay_end = limit.min(to.saturating_add(reach));
            let found = automaton
                .find(&text[from..hay_end])
                .map(|m| from + m.start())
                .filter(|&p| p < to);
            // The first start a non-ASCII byte the walk reads admits. From
            // `p + reach` on such a byte only admits starts after `p`. The
            // bound of its character is the whole text, which admits more
            // than the match's own bound.
            let scan_end = found.map_or(hay_end, |p| p.saturating_add(reach).min(hay_end));
            let folded = (!text[from..scan_end].is_ascii())
                .then(|| {
                    (from..scan_end)
                        .find(|&q| text[q] >= 0x80 && trie.reads_non_ascii_at(text, q, text.len()))
                })
                .flatten()
                .map(|q| q.saturating_sub(reach).max(from))
                .filter(|&p| p < to);
            if let Some(p) = folded.or(found) {
                return Some(p);
            }
            from = to;
            window = window.saturating_mul(2);
        }
        None
    }
}

/// Leading classes a [`SearchJump`] checks.
const MAX_PREFIX: usize = 16;

/// Bytes in decreasing order of frequency in prose and source code.
pub(crate) const COMMON_BYTES: &[u8] =
    b" etaoinsrhldcumfpgwybvkxjqz\nETAOINSRHLDCUMFPGWYBVKXJQZ.,;:()\"'_-=/0123456789{}[]<>*#@$%&!?+|\\~^`\t";

/// How common a byte is: higher is more frequent.
fn byte_rank(byte: u8) -> usize {
    COMMON_BYTES
        .iter()
        .position(|&b| b == byte)
        .map_or(0, |at| COMMON_BYTES.len() - at)
}

pub(crate) fn plan_jump(reg: &RegexType) -> Option<SearchJump> {
    if !onigenc_is_ascii_compatible_encoding(reg.enc)
        || reg.extp.as_ref().is_some_and(|ext| ext.callout_num != 0)
    {
        return None;
    }
    // C's optimizer already searches an exact string at the match start; with
    // a line anchor it also checks the line start of each occurrence.
    let exact_at_start = matches!(
        reg.optimize,
        OptimizeType::Str | OptimizeType::StrFast | OptimizeType::StrFastStepForward
    ) && reg.dist_max == 0;
    let mut pc = 0;
    let mut line_start = false;
    loop {
        match reg.ops.get(pc)?.opcode {
            OpCode::MemStart | OpCode::MemStartPush | OpCode::Mark => {}
            OpCode::BeginLine if pc == 0 || !line_start => line_start = true,
            OpCode::WordBoundary | OpCode::NoWordBoundary => {}
            _ => break,
        }
        pc += 1;
    }
    if exact_at_start && (reg.sub_anchor & ANCR_BEGIN_LINE) != 0 {
        line_start = false;
    }
    // A positive look-behind at an ASCII literal holds at `p` exactly when
    // the literal ends at `p`: ASCII bytes are character heads, so stepping
    // back its characters lands on its first byte.
    if let (
        Some(OperationPayload::LookBehindOp {
            char_len,
            not: false,
        }),
        Some(literal),
    ) = (
        reg.ops.get(pc).map(|op| &op.payload),
        reg.ops.get(pc + 1).and_then(exact_bytes),
    ) {
        if !literal.is_empty() && literal.is_ascii() && literal.len() == *char_len as usize {
            let mut prefix = Vec::new();
            collect_prefix(reg, pc + 2, &mut prefix);
            return Some(SearchJump {
                prefix,
                behind: Some(memchr::memmem::Finder::new(literal).into_owned()),
                ..SearchJump::lines_if(line_start)
            });
        }
    }
    if let Some(OperationPayload::AltLiterals { trie_idx }) = reg.ops.get(pc).map(|op| &op.payload)
    {
        let trie = reg.literal_tries.get(*trie_idx as usize)?;
        // A bare case-sensitive alternation takes `ac_alt` already.
        if reg.ac_alt.is_some() {
            return line_start.then(SearchJump::lines);
        }
        // Folded tries hold ASCII literals, which Aho-Corasick folds.
        let reach = if trie.is_case_insensitive() {
            let longest = trie.literals().iter().map(Vec::len).max()?;
            if longest == 0 || trie.literals().iter().any(|literal| !literal.is_ascii()) {
                return line_start.then(SearchJump::lines);
            }
            Some(longest - 1)
        } else {
            None
        };
        return Some(SearchJump {
            alternation: Some(Alternation {
                trie_idx: *trie_idx as usize,
                reach,
                automaton: std::sync::OnceLock::new(),
            }),
            ..SearchJump::lines_if(line_start)
        });
    }
    let mut prefix = Vec::new();
    collect_prefix(reg, pc, &mut prefix);
    let probe = (prefix.len() >= 2 && !exact_at_start)
        .then(|| {
            prefix
                .iter()
                .enumerate()
                // An empty class, such as `[a&&b]`, has no byte to find.
                .filter(|(_, class)| (1..=3).contains(&class_members(class).count()))
                .min_by_key(|(_, class)| class_members(class).map(byte_rank).sum::<usize>())
                .map(|(at, _)| at)
        })
        .flatten();
    let Some(probe) = probe else {
        return line_start.then(SearchJump::lines);
    };
    // A non-ASCII probe byte could sit inside a multibyte character.
    if prefix
        .iter()
        .any(|class| class_members(class).any(|b| b >= 0x80))
    {
        return None;
    }
    let probe_bytes = class_members(&prefix[probe]).collect();
    Some(SearchJump {
        prefix,
        probe,
        probe_bytes,
        ..SearchJump::lines_if(line_start)
    })
}

/// The leading byte classes and literals from `pc` on, at most
/// [`MAX_PREFIX`] of them.
fn collect_prefix(reg: &RegexType, mut pc: usize, prefix: &mut Vec<BitSet>) {
    while prefix.len() < MAX_PREFIX {
        let Some(op) = reg.ops.get(pc) else { break };
        if let Some(literal) = exact_bytes(op) {
            prefix.extend(literal.iter().map(|&b| single_byte(b)));
        } else if let (OpCode::CClass, OperationPayload::CClass { bsp, .. }) =
            (op.opcode, &op.payload)
            && bsp[128 / BITS_IN_ROOM..].iter().all(|&bits| bits == 0)
        {
            prefix.push(**bsp);
        } else {
            break;
        }
        pc += 1;
    }
    prefix.truncate(MAX_PREFIX);
}

/// The bytes an exact string instruction matches.
fn exact_bytes(op: &crate::regint::Operation) -> Option<&[u8]> {
    match (op.opcode, &op.payload) {
        (
            OpCode::Str1 | OpCode::Str2 | OpCode::Str3 | OpCode::Str4 | OpCode::Str5,
            OperationPayload::Exact { s },
        ) => Some(&s[..op.opcode as usize - OpCode::Str1 as usize + 1]),
        (OpCode::StrN, OperationPayload::ExactN { s, n }) => Some(&s[..*n as usize]),
        _ => None,
    }
}

fn single_byte(byte: u8) -> BitSet {
    let mut set = [0; BITSET_REAL_SIZE];
    bitset_set_bit(&mut set, byte as usize);
    set
}

fn class_members(class: &BitSet) -> impl Iterator<Item = u8> + '_ {
    (0..SINGLE_BYTE_SIZE)
        .filter(|&b| bitset_at(class, b))
        .map(|b| b as u8)
}

impl SearchJump {
    /// Only the line-start check.
    fn lines() -> Self {
        Self::lines_if(true)
    }

    fn lines_if(line_start: bool) -> Self {
        SearchJump {
            line_start,
            prefix: Vec::new(),
            probe: 0,
            probe_bytes: Vec::new(),
            alternation: None,
            behind: None,
        }
    }

    /// Whether `s` is a line start as `BeginLine` decides: the buffer start
    /// (unless NOTBOL), or after a newline, but never the end.
    fn is_line_start(text: &[u8], s: usize, end: usize, notbol: bool) -> bool {
        if s == 0 {
            return !notbol;
        }
        if s >= end {
            return false;
        }
        // `onigenc_get_prev_char_head` steps back over 0x80..=0xBF in every
        // encoding.
        let mut head = s - 1;
        while head > 0 && (text[head] & 0xC0) == 0x80 {
            head -= 1;
        }
        text[head] == b'\n'
    }

    fn prefix_at(&self, text: &[u8], p: usize, limit: usize) -> bool {
        p + self.prefix.len() <= limit
            && self
                .prefix
                .iter()
                .zip(&text[p..])
                .all(|(class, &b)| bitset_at(class, b as usize))
    }

    /// The first position from `s` on where an attempt can pass the leading
    /// checks: a line start for `^`, the leading classes within `limit`.
    /// `None` when no such position remains.
    fn candidate(
        &self,
        reg: &RegexType,
        text: &[u8],
        mut s: usize,
        end: usize,
        limit: usize,
        notbol: bool,
    ) -> Option<usize> {
        loop {
            let p = if let Some(alternation) = &self.alternation {
                if s >= limit {
                    return None;
                }
                alternation.candidate(reg, text, s, limit)?
            } else if let Some(behind) = &self.behind {
                // The next occurrence ending at or after `s`.
                if s > limit {
                    return None;
                }
                let len = behind.needle().len();
                let from = s.saturating_sub(len);
                from + behind.find(&text[from..limit])? + len
            } else if self.probe_bytes.is_empty() {
                s
            } else {
                let from = s.checked_add(self.probe)?;
                if from >= limit {
                    return None;
                }
                let hay = &text[from..limit];
                let hit = match *self.probe_bytes.as_slice() {
                    [a] => memchr::memchr(a, hay),
                    [a, b] => memchr::memchr2(a, b, hay),
                    [a, b, c] => memchr::memchr3(a, b, c, hay),
                    _ => unreachable!("probe classes hold one to three bytes"),
                }?;
                from + hit - self.probe
            };
            if !self.prefix.is_empty() && !self.prefix_at(text, p, limit) {
                s = p + 1;
                continue;
            }
            if self.line_start && !Self::is_line_start(text, p, end, notbol) {
                if p >= end {
                    return None;
                }
                // The next line starts after the next newline.
                s = p + 1 + memchr::memchr(b'\n', &text[p..end])?;
                continue;
            }
            return Some(p);
        }
    }

    /// Where the search may continue instead of `s`: the next position that
    /// can pass the leading checks, if the plain loop would step onto it.
    /// Between `s` and that position every attempt fails its first check.
    /// Beyond a byte the scan does not vouch for (not valid UTF-8), the plain
    /// loop's steps could pass over the position, so the search stays at `s`.
    ///
    /// `last` bounds a position loop whose final attempt is the first
    /// character boundary at or past it: a candidate past `last` is not
    /// attempted there, so the search moves to `last` itself when that is a
    /// boundary, and stays otherwise. Without `last` (the optimizer's windows
    /// decide), the search moves past the text when no candidate remains.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn next_start(
        &self,
        reg: &RegexType,
        text: &[u8],
        s: usize,
        end: usize,
        limit: usize,
        options: crate::oniguruma::OnigOptionType,
        last: Option<usize>,
    ) -> usize {
        let enc = reg.enc;
        let notbol = options.contains(crate::oniguruma::OnigOptionType::NOTBOL);
        let candidate = self.candidate(reg, text, s, end, limit, notbol);
        let target = match (candidate, last) {
            (Some(p), Some(last)) if p > last => last,
            (None, Some(last)) => last,
            (Some(p), _) => p,
            (None, None) => end,
        };
        let boundary = target >= end || enc.max_enc_len() == 1 || (text[target] & 0xC0) != 0x80;
        if target > s && boundary && reachable(enc, text, s, target) {
            target
        } else {
            s
        }
    }
}

/// Whether the plain loop, stepping from character head `s`, steps onto `p`:
/// always in a single-byte encoding, and over valid UTF-8.
fn reachable(enc: OnigEncoding, text: &[u8], s: usize, p: usize) -> bool {
    // Mostly ASCII, which `is_ascii` reads a word at a time.
    enc.max_enc_len() == 1 || text[s..p].is_ascii() || std::str::from_utf8(&text[s..p]).is_ok()
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
        reg.search_jump = None;
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
    fn whole_lookahead_runs_keep_restored_starts_and_ascii_guards() {
        for pattern in [
            r"(?=\w?[-\w\s]*\b(?:class|(?<!@)interface|enum)\s+[$\w]+)",
            r"(?=\w?[\w\s]*\brecord\s+[$\w]+)",
        ] {
            let reg = compile(pattern, UTF8).unwrap();
            let run = reg.leading_run.as_ref().expect("whole assertion run");
            assert!(run.optional_word_prefix && run.ascii_only && run.min_zero);
        }
        for pattern in [
            r"(?=\w?[a-z]*record)",
            r"(?=\w?[\w\s]*record)b",
            r"(?=(?>\w?[\w\s]*)record)",
        ] {
            assert!(
                compile(pattern, UTF8).unwrap().leading_run.is_none(),
                "{pattern}"
            );
        }
        // A successful lookahead restores the original attempt before `b`.
        // Failure at `a` therefore says nothing about a later start at `b`.
        let reg = compile(r"(?=[a-z]*a)b", UTF8).unwrap();
        assert!(reg.leading_run.is_none());
        let actual = search(
            &reg,
            b"aba",
            (3, 0, 3),
            ONIG_OPTION_NONE,
            &onig_new_match_param(),
        );
        assert_eq!(actual, (1, vec![1], vec![2]));
        // With an unbounded optimizer range, an earlier word start can
        // pass the assertion and fail its suffix before a later word wins.
        let reg = compile(r"(?=[\w\s]*a)\b\w+", UTF8).unwrap();
        assert!(reg.leading_run.is_none());
        let actual = search(
            &reg,
            b"xx aba",
            (6, 1, 6),
            ONIG_OPTION_NONE,
            &onig_new_match_param(),
        );
        assert_eq!(actual, (3, vec![3], vec![6]));
    }

    /// A run whose first character is narrower than its class skips the
    /// later starts of a run only after an attempt that read that character.
    /// Without the start map, an attempt at a digit fails at `[A-Z_a-z]` and
    /// stands for nothing: `9abc :` still matches at 1.
    #[test]
    fn narrower_first_character_skips_only_after_reading_it() {
        let mp = onig_new_match_param();
        for pattern in [
            r"[A-Z_a-z]\w*\s*:",
            r"([A-Z_a-z]\w*)\s*:",
            r"[a-c][a-z]*\s*:",
            r"([A-Z_a-z][_\w\d]*)\s*:",
            r"[_a-zé][0-9_a-zé]*\s*:",
        ] {
            let mut reg = compile(pattern, UTF8).unwrap();
            assert!(
                reg.leading_run
                    .as_ref()
                    .is_some_and(|run| run.head.is_some()),
                "{pattern}"
            );
            reg.search_start_map = None;
            reg.search_jump = None;
            reg.has_first_byte_map = false;
            let reference = reference(pattern, UTF8);
            for text in [
                &b"9abc :x"[..],
                b"99ab c: d9e:",
                b"x9 _a:",
                "9é1éb :".as_bytes(),
                "é9ab:".as_bytes(),
            ] {
                let bounds = (text.len(), 0, text.len());
                assert_eq!(
                    search(&reg, text, bounds, ONIG_OPTION_NONE, &mp),
                    search(&reference, text, bounds, ONIG_OPTION_NONE, &mp),
                    "{pattern} on {text:?}"
                );
            }
        }
    }

    /// The starts a literal after a zero-width and class repetition part
    /// leaves (`LiteralPrefix`) give the plain loop's results, regions and
    /// limit errors, for every start and range.
    /// `window` keeps every start up to `last` whose class run reaches an
    /// occurrence, however far past `last` that occurrence lies.
    #[test]
    fn literal_prefix_window_keeps_reaching_starts() {
        let reg = compile(r"[ \t]*//", UTF8).unwrap();
        let prefix = reg.literal_prefix.as_deref().unwrap();
        let reaches = |text: &[u8], x: usize| {
            let run = text[x..]
                .iter()
                .take_while(|&&b| b == b' ' || b == b'\t')
                .count();
            text[x + run..].starts_with(b"//")
        };
        for gap in 0..80 {
            for tail in [&b""[..], b"/", b" //"] {
                let text = [&b"x"[..], &b" \t".repeat(gap)[..gap], b"//", tail].concat();
                let end = text.len();
                for s in 0..=end {
                    for last in s..=end {
                        let window = prefix.window(reg.enc, &text, s, last, end);
                        let kept = window.map_or(last + 1, |(first, _)| first.min(last + 1));
                        assert!(
                            (s..kept).all(|x| !reaches(&text, x)),
                            "{text:?} s={s} last={last} {window:?}"
                        );
                        if let Some((_, k)) = window.filter(|&(_, k)| k != usize::MAX) {
                            assert!(text[k..].starts_with(b"//"), "{text:?} s={s} k={k}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn literal_prefix_starts_match_the_plain_loop() {
        let _lock = crate::regexec::LIMIT_TEST_LOCK.lock().unwrap();
        let patterns = [
            r"(?<!\+\+|--)(?<=[!(+,:=>?\[]|^await|[^$._[:alnum:]]await|^return)\s*(\{)",
            r"(?:^|(?<=[&(,]|[;\s]if\s))\s*((/))(?![*+?{}])",
            r"\s*(;)",
            r"[ \t]*//",
            r"\s*,\s*",
            r"(?i)\s*(text:)",
            r"(?<=\s)\s*=>",
            r"\b\s*(\()",
            r"[^a-z]*(z)",
            r"(?=a)\s*b(c)",
        ];
        // The literal's byte lies in the class, or a branch reads before it:
        // no plan.
        for pattern in [r"\S*;", r"\w*x", r"a*(?:b|c)d"] {
            assert!(
                compile(pattern, UTF8).unwrap().literal_prefix.is_none(),
                "{pattern}"
            );
        }
        // A leading-check jump lands on fewer starts: it keeps the search.
        for pattern in [r"(?<=x)\s*(\{)", r"^\s*(\.)", r"^([-0-9A-Za-z]+)(:)\s*"] {
            let reg = compile(pattern, UTF8).unwrap();
            assert!(
                reg.search_jump.is_some() && reg.literal_prefix.is_none(),
                "{pattern}"
            );
        }
        let inputs: &[&[u8]] = &[
            b"",
            b"{",
            b"return {a}; x {",
            b"if /x/ , /y/ ;z",
            "a\u{a0}\u{3000}{ ,\u{a0}; \u{2028}//".as_bytes(),
            b"\xe0 { \xc3; \xff//x",
            b"text: TEXT :tex: ;",
            b"a => b=>c ( d(",
            b"abcz 12z\xc3\xa9z",
        ];
        let mut comparisons = 0;
        for pattern in patterns {
            let reg = compile(pattern, UTF8).unwrap();
            let prefix = reg.literal_prefix.as_deref().expect(pattern);
            let mut plain = compile(pattern, UTF8).unwrap();
            plain.literal_prefix = None;
            // Per-match limits up to and just past the bound of a left-out
            // attempt's backtracks: below it every attempt runs, above it
            // no left-out attempt could have reached the limit.
            let bound = prefix.retry_bound.expect(pattern);
            let mut limits = vec![(0, 0, 0), (0, 3, 0), (0, 0, 3), (10_000_000, 0, 0)];
            limits.extend(
                (1..=12)
                    .chain(bound.saturating_sub(2)..=bound + 3)
                    .map(|r| (r, 0, 0)),
            );
            for &(retry, search_retry, stack) in &limits {
                let mut mp = onig_new_match_param();
                mp.retry_limit_in_match = retry;
                mp.retry_limit_in_search = search_retry;
                mp.match_stack_limit = stack;
                for text in inputs {
                    for start in 0..=text.len() {
                        for range in start..=text.len() {
                            let bounds = (text.len(), start, range);
                            assert_eq!(
                                search(&reg, text, bounds, ONIG_OPTION_NONE, &mp),
                                search(&plain, text, bounds, ONIG_OPTION_NONE, &mp),
                                "{pattern} on {text:?} {bounds:?} limits {retry}/{search_retry}/{stack}"
                            );
                            comparisons += 1;
                        }
                    }
                }
            }
        }
        assert!(comparisons > 10_000, "{comparisons}");
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
            r"\w*@",
            r"\s*,\s*",
            r"\b\w+@",
            r"\b\w+\(",
            r"\b[a-z_]+:",
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
            r"\b[\w.]+@",
            r"\b\w*@",
            r"(\w+)\1",
            r"(\w+)@(?(1)a|b)",
            r"\w+@\G",
            r"\w+\K@",
            r"(?<n>\w+)@\g<n>",
            r"\w+@(?{x})",
            r"a+@",
            r"(?:\w|-)+@",
            r"\w{2,}@",
            r"\A\w+@",
        ] {
            assert_eq!(plan(pattern), (false, false), "{pattern}");
        }
    }

    /// A leading empty class used to become the probe: the jump then never
    /// found a candidate and never gave up.
    #[test]
    fn jumps_past_an_empty_leading_class_end() {
        let mp = onig_new_match_param();
        for pattern in [r"(?i)f[a&&b]", r"[a-z][a&&b]", r"[ab]c[a&&b]"] {
            for text in [&b"abc XYZ 123"[..], b"", b"fff abc"] {
                let reg = compile(pattern, UTF8).unwrap();
                let bounds = (text.len(), 0, text.len());
                let expected = search(
                    &reference(pattern, UTF8),
                    text,
                    bounds,
                    ONIG_OPTION_NONE,
                    &mp,
                );
                assert_eq!(expected.0, ONIG_MISMATCH, "{pattern}");
                assert_eq!(
                    search(&reg, text, bounds, ONIG_OPTION_NONE, &mp),
                    expected,
                    "{pattern}"
                );
            }
        }
    }

    /// Over malformed UTF-8 no jump is taken: a lead byte's step may pass
    /// over a candidate right before the end.
    #[test]
    fn jumps_over_malformed_bytes_keep_the_loop_steps() {
        let mp = onig_new_match_param();
        for pattern in [r"(?<=\()x", r"(?<=\()x?", r"^\s*x"] {
            let reg = compile(pattern, UTF8).unwrap();
            assert!(reg.search_jump.is_some(), "{pattern}");
            let reference = reference(pattern, UTF8);
            for text in [
                &b"\xe0(x"[..],
                b"\xe0(",
                b"a\xf0(x",
                b"\xe0\nx",
                b"\xff\xe0",
            ] {
                for start in 0..=text.len() {
                    for range in start..=text.len() {
                        let bounds = (text.len(), start, range);
                        assert_eq!(
                            search(&reg, text, bounds, ONIG_OPTION_NONE, &mp),
                            search(&reference, text, bounds, ONIG_OPTION_NONE, &mp),
                            "{pattern} on {text:?} {bounds:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn jumps_follow_the_leading_checks() {
        let jump = |pattern: &str| {
            compile(pattern, UTF8)
                .unwrap()
                .search_jump
                .map(|jump| (jump.line_start, jump.prefix.len(), jump.probe_bytes))
        };
        // The rarest class is probed: `x`/`X` in "regex".
        assert_eq!(jump(r"(?i)regex"), Some((false, 5, b"Xx".to_vec())));
        // An empty class is never probed: there is no byte to find.
        assert_eq!(jump(r"(?i)f[a&&b]"), Some((false, 2, b"Ff".to_vec())));
        assert_eq!(jump(r"[a-z][a&&b]"), None);
        assert_eq!(jump(r"(?i)\berror\b").map(|(_, len, _)| len), Some(5));
        assert_eq!(jump(r"^\s*//"), Some((true, 0, Vec::new())));
        assert_eq!(
            jump(r"^ab"),
            // C's optimizer finds "ab" and checks its line start itself.
            None
        );
        // C's optimizer already searches the leading string, or nothing is
        // known about the leading characters.
        let alternation = |pattern: &str| {
            compile(pattern, UTF8)
                .unwrap()
                .search_jump
                .is_some_and(|jump| jump.alternation.is_some())
        };
        assert!(alternation(r"\b(?:fn|let|for|while|match)\b"));
        // A folded trie keeps the starts near non-ASCII input as candidates.
        let reach = |pattern: &str| {
            compile(pattern, UTF8)
                .unwrap()
                .search_jump
                .and_then(|jump| jump.alternation.map(|alternation| alternation.reach))
        };
        assert_eq!(reach(r"(?i)\b(?:fn|let|for|while|match)\b"), Some(Some(4)));
        assert_eq!(reach(r"\b(?:fn|let|for|while|match)\b"), Some(None));
        // A positive look-behind at an ASCII literal anchors the search.
        let behind = |pattern: &str| {
            compile(pattern, UTF8)
                .unwrap()
                .search_jump
                .and_then(|jump| jump.behind.map(|finder| finder.needle().to_vec()))
        };
        assert_eq!(behind(r"(?<=took=)\d+"), Some(b"took=".to_vec()));
        assert_eq!(behind(r"\b(?<=id=)\d+"), Some(b"id=".to_vec()));
        assert_eq!(behind(r"(?<!took=)\d+"), None);
        assert_eq!(behind(r"(?<=[=:])\w+"), None);
        assert_eq!(behind(r"(?<=é)\w+"), None);
        assert!(jump("ERROR").is_none());
        assert!(jump(r"\d+").is_none());
        assert!(jump(r"(?i)k").is_none());
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
            r"\s*,\s*",
            r"\b\w+\(",
            r"\b[a-z]+:",
            r"(?i)regular ex",
            r"(?i)\bab\b",
            r"^\s*//",
            r"^ab",
            r"(?m)^a+$",
            r"(?=ab)a",
            r"(?>ab)c",
            r"\bab",
            r"x*:",
            r"\p{L}*@",
            r"\b(?:ab|abc|b|xx:)\b",
            r"(?:ab|cd|ex|g:)(\w?)",
            r"^(?:ab|re|x:)",
            r"(?<=ab)\w+",
            r"(?<=x:)(\d?)",
            r"\b(?<=ab:)c",
            r"(?<=ab)(?=c)",
            r"^(?<=a)b",
            r"(?<=é)\w",
            r"(?<=aé)a",
            r"(?<=\xc3)a",
            r"(?i)(?:ks|ss|st|ff|re)",
            r"(?i)\b(?:k|ss|stx|ffi)(\w?)",
            // A first character narrower than the run: a start on a digit
            // fails at it and says nothing about the starts after it.
            r"[A-Z_a-z]\w*(?=\s*:)",
            r"([A-Z_a-z]\w*)\s*(?=[(;=])(;)?",
            r"[a-c][a-z]*:",
            r"(?>[A-Z_a-z]\w*):",
            r"\b[A-Z_a-z]\w*\(",
            r"[a-z][\w.]*@",
            r"([A-Z_a-z][_\w\d]*)\s*(?=[(;=])",
            r"[_a-zé][0-9_a-zé]*(?=\s*:)",
        ];
        let inputs: &[&[u8]] = &[
            b"9abc :x 1y: _z9 ( q;=",
            b"9a.b@ 1ab: x9(",
            "9é1éb :x é9a(".as_bytes(),
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
            b"a\nab\n  //x\n",
            b"//\n ab:\n",
            b"Regular EX regular ex",
            b" a , b,c",
            b"f(x) g (y)",
            b"ab\n\x80ab",
            b"\n\xc3\xa9//",
            b"xx: :",
            b"\xc3\xff@a",
            // Unicode case folding: Kelvin sign, sharp s, long s, ligatures.
            "xK\u{212a}s \u{df}t\u{17f}s\u{fb00}i".as_bytes(),
            "RE \u{fb06}x Ss\u{fb03}".as_bytes(),
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
                                    ONIG_OPTION_NOTBOL,
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

    /// Folded alternations over subjects longer than the scan windows, with
    /// Unicode case folds and malformed bytes around every window edge.
    #[test]
    fn folded_alternations_find_every_match() {
        let patterns = [
            r"(?i)(?:error|warn|fatal|panic)",
            r"(?i)\b(?:kiss|strasse|office|k)\b",
            r"(?i)(?:ss|st|ffl|re)(\w?)",
        ];
        let fillers: &[&[u8]] = &[b"-", b"x", b" ", "\u{e9}".as_bytes(), b"\xff", b"\xe2\x84"];
        let words: &[&[u8]] = &[
            b"ERROR",
            b"Warn",
            "\u{212a}iss".as_bytes(),
            "stra\u{df}e".as_bytes(),
            "o\u{fb03}ce".as_bytes(),
            "\u{17f}t".as_bytes(),
            "\u{fb04}".as_bytes(),
            b"PaNiC",
            b"k",
        ];
        for pattern in patterns {
            let optimized = compile(pattern, UTF8).unwrap();
            assert!(
                optimized
                    .search_jump
                    .as_ref()
                    .is_some_and(|jump| jump.alternation.is_some()),
                "{pattern}"
            );
            let reference = reference(pattern, UTF8);
            for (filler, &word) in fillers.iter().zip(words.iter().cycle()) {
                for gap in [1, 7, 250, 255, 256, 257, 300, 700] {
                    let mut text = Vec::new();
                    for (at, &other) in words.iter().enumerate() {
                        text.extend(filler.repeat(gap + at));
                        text.extend_from_slice(if at % 2 == 0 { word } else { other });
                    }
                    assert_eq!(
                        scan(&optimized, &text),
                        scan(&reference, &text),
                        "{pattern:?} {filler:?} {gap}"
                    );
                }
            }
        }
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
        let wrappers = [
            "{}",
            "({})",
            "(?>{})",
            "(?:{})",
            "(?<n>{})",
            "(({}))",
            "^{}",
            r"\b{}",
            "(?i:ab){}",
            "(?i)re{}",
            "(?:ab|ing|x@|re){}",
            "(?i)(?:ab|ss|k|re){}",
            "(?<=ab){}",
            "(?<=:)(?:{})",
            r"\b(?:ab|ing|re|:=){}",
        ];
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
            b"AB",
            b"Re",
            b"rE",
            b"\n\x80",
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
            "\u{212a}".as_bytes(),
            "\u{df}".as_bytes(),
            b"SS",
            b"K",
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
