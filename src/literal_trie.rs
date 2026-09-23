//! Compact trie for matching literal alternations.
//!
//! When a regex alternation consists entirely of literal strings (e.g.
//! `accent-color|additive-symbols|...|z-index`), we compile them into a trie
//! for O(len) matching instead of O(n*len) backtracking.

use crate::encodings::utf8::ONIG_ENCODING_UTF8;
use crate::regenc::Encoding;

/// Compact trie over the literal alternatives of one alternation.
pub struct LiteralTrie {
    nodes: Vec<TrieNode>,
    case_insensitive: bool,
    raw_literals: Vec<Vec<u8>>,
    /// Unicode case folding of a case-insensitive UTF-8 alternation.
    folds: Option<Box<CaseFolds>>,
}

struct TrieNode {
    /// Sorted (byte, child_index) pairs for binary search lookup.
    children: Vec<(u8, u32)>,
    /// Index of the literal that ends at this node, if any.
    terminal: Option<u32>,
}

/// What a case-insensitive alternation of ASCII literals accepts beyond
/// ASCII case pairs, as the engine compiles it without a trie.
///
/// `unravel_case_fold_string` turns each literal into single-character
/// classes and, where a multi-character fold starts (`ss`, `st`, `ffi`),
/// an alternation of exact strings covering that segment. The trie walks
/// the input folded to lowercase ASCII and, when it meets non-ASCII input,
/// checks the literal it reached against those segments. The data is
/// derived from the same case-fold queries (see `literal_trie_case_folds`
/// in `regcomp`).
#[derive(Debug, Default)]
pub(crate) struct CaseFolds {
    /// Code points a single-character class accepts from non-ASCII input,
    /// with the lowercase letter of that class, sorted by code point. A
    /// class compiled with multibyte members decodes such input, including
    /// malformed sequences, and also tests the decoded code against its
    /// ASCII members.
    pub(crate) class_members: Vec<(u32, u8)>,
    /// Characters only a multi-character segment accepts, as exact UTF-8
    /// bytes, with the lowercase text they stand for (`ß` for `ss`).
    pub(crate) ligatures: Vec<(Vec<u8>, Vec<u8>)>,
    /// Multi-character segments of each literal, in order: start offset,
    /// length, and index into `accepted`.
    pub(crate) segments: Vec<Vec<(usize, usize, usize)>>,
    /// The exact inputs each distinct multi-character segment accepts.
    pub(crate) accepted: Vec<Vec<Vec<u8>>>,
}

/// A character consumed by the folded walk: its input bytes and how many
/// literal bytes (lowercase ASCII symbols) it stands for.
#[derive(Clone, Copy)]
struct FoldedChar {
    start: usize,
    end: usize,
    symbols: usize,
}

impl CaseFolds {
    fn class_member(&self, code: u32) -> Option<u8> {
        self.class_members
            .binary_search_by_key(&code, |&(member, _)| member)
            .ok()
            .map(|at| self.class_members[at].1)
    }

    fn ligature(&self, bytes: &[u8]) -> Option<&[u8]> {
        self.ligatures
            .iter()
            .find(|(ligature, _)| ligature == bytes)
            .map(|(_, text)| text.as_slice())
    }

    /// Whether the characters consumed for `literal` fit its segments: a
    /// multi-character segment must be covered by whole characters whose
    /// bytes it accepts, and every other offset by one single-symbol
    /// character (which the walk already matched against its class).
    fn accepts(&self, literal: usize, input: &[u8], chars: &[FoldedChar]) -> bool {
        let mut at = 0;
        let mut offset = 0;
        for &(start, len, accepted) in &self.segments[literal] {
            while offset < start {
                match chars.get(at) {
                    Some(c) if c.symbols == 1 => {}
                    _ => return false,
                }
                offset += 1;
                at += 1;
            }
            let Some(first) = chars.get(at) else {
                return false;
            };
            let mut covered = 0;
            let mut span_end = first.start;
            while covered < len {
                let Some(c) = chars.get(at) else {
                    return false;
                };
                covered += c.symbols;
                span_end = c.end;
                at += 1;
            }
            if covered != len
                || !self.accepted[accepted]
                    .iter()
                    .any(|candidate| candidate.as_slice() == &input[first.start..span_end])
            {
                return false;
            }
            offset += len;
        }
        chars[at..].iter().all(|c| c.symbols == 1)
    }
}

impl LiteralTrie {
    /// Build a trie from a set of literal byte strings.
    /// If `case_insensitive`, all keys are lowercased during insertion
    /// and lookups will also lowercase input bytes.
    pub fn build(literals: &[&[u8]], case_insensitive: bool) -> Self {
        let mut trie = LiteralTrie {
            nodes: vec![TrieNode {
                children: Vec::new(),
                terminal: None,
            }],
            case_insensitive,
            raw_literals: literals.iter().map(|l| l.to_vec()).collect(),
            folds: None,
        };

        for (index, lit) in literals.iter().enumerate() {
            trie.insert(lit, index as u32);
        }

        trie
    }

    /// Build a case-insensitive trie over ASCII literals that also accepts
    /// the non-ASCII input described by `folds`, whose segments are indexed
    /// like `literals`.
    pub(crate) fn build_folded(literals: &[&[u8]], folds: CaseFolds) -> Self {
        let mut trie = Self::build(literals, true);
        trie.folds = Some(Box::new(folds));
        trie
    }

    fn insert(&mut self, key: &[u8], index: u32) {
        let mut node_idx: u32 = 0;
        for &b in key {
            let b = if self.case_insensitive {
                b.to_ascii_lowercase()
            } else {
                b
            };
            let children = &self.nodes[node_idx as usize].children;
            match children.binary_search_by_key(&b, |&(k, _)| k) {
                Ok(pos) => {
                    node_idx = children[pos].1;
                }
                Err(pos) => {
                    let new_idx = self.nodes.len() as u32;
                    self.nodes.push(TrieNode {
                        children: Vec::new(),
                        terminal: None,
                    });
                    self.nodes[node_idx as usize]
                        .children
                        .insert(pos, (b, new_idx));
                    node_idx = new_idx;
                }
            }
        }
        // A repeated literal keeps its first position in the alternation.
        self.nodes[node_idx as usize].terminal.get_or_insert(index);
    }

    #[inline]
    fn child(&self, node: u32, symbol: u8) -> Option<u32> {
        let children = &self.nodes[node as usize].children;
        children
            .binary_search_by_key(&symbol, |&(k, _)| k)
            .ok()
            .map(|at| children[at].1)
    }

    /// Returns the raw literals that were used to build this trie.
    pub fn literals(&self) -> &[Vec<u8>] {
        &self.raw_literals
    }

    /// Returns whether this trie was built with case-insensitive matching.
    pub fn is_case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    /// Try to find the longest matching literal starting at `input[pos]`.
    /// Returns the match length if found, or `None`.
    pub fn find_match(&self, input: &[u8], pos: usize, end: usize) -> Option<usize> {
        let mut longest = None;
        self.for_each_match(input, pos, end, |_, len| longest = Some(len));
        longest
    }

    /// The match of the literal that comes first in the alternation, and
    /// whether other literals match too. Ordered alternation tries that
    /// literal first; the others are its backtracking alternatives.
    #[inline]
    pub fn first_match(&self, input: &[u8], pos: usize, end: usize) -> Option<(usize, bool)> {
        let mut first: Option<(u32, usize)> = None;
        let mut others = false;
        self.for_each_match(input, pos, end, |literal, len| match first {
            Some((best, _)) => {
                others = true;
                if literal < best {
                    first = Some((literal, len));
                }
            }
            None => first = Some((literal, len)),
        });
        first.map(|(_, len)| (len, others))
    }

    /// Every match at `pos` as `(literal, length)`, in alternation order.
    pub fn matches_in_order(&self, input: &[u8], pos: usize, end: usize) -> Vec<(u32, usize)> {
        let mut matches = Vec::new();
        self.for_each_match(input, pos, end, |literal, len| matches.push((literal, len)));
        matches.sort_unstable();
        matches
    }

    /// Calls `found(literal, length)` for every literal matching at `pos`,
    /// shortest first. At most one match per literal: its folded text fixes
    /// how much input it covers.
    #[inline]
    fn for_each_match(
        &self,
        input: &[u8],
        pos: usize,
        end: usize,
        mut found: impl FnMut(u32, usize),
    ) {
        let mut node = 0;
        let mut i = pos;
        loop {
            if let Some(literal) = self.nodes[node as usize].terminal {
                found(literal, i - pos);
            }
            if i >= end {
                return;
            }
            let b = input[i];
            if b >= 0x80 {
                if let Some(folds) = &self.folds {
                    // Every ASCII case variant of a literal is accepted, so
                    // only input from the first non-ASCII byte on needs the
                    // folded walk.
                    self.for_each_folded_match(folds, input, pos, end, i, node, &mut found);
                    return;
                }
            }
            let symbol = if self.case_insensitive {
                b.to_ascii_lowercase()
            } else {
                b
            };
            match self.child(node, symbol) {
                Some(next) => node = next,
                None => return,
            }
            i += 1;
        }
    }

    /// The rest of a folded walk from the non-ASCII byte at `i`, reached at
    /// `node` over ASCII input: each character reads as the letter of a
    /// class that accepts it or as a ligature's text, and every literal
    /// reached is checked against its segments.
    #[cold]
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn for_each_folded_match(
        &self,
        folds: &CaseFolds,
        input: &[u8],
        pos: usize,
        end: usize,
        mut i: usize,
        mut node: u32,
        found: &mut dyn FnMut(u32, usize),
    ) {
        let mut chars: Vec<FoldedChar> = (pos..i)
            .map(|at| FoldedChar {
                start: at,
                end: at + 1,
                symbols: 1,
            })
            .collect();
        while i < end {
            let b = input[i];
            let mut letter = [0u8];
            let (symbols, len): (&[u8], usize) = if b < 0x80 {
                letter[0] = b.to_ascii_lowercase();
                (&letter, 1)
            } else {
                // As a class instruction reads a character: length from the
                // lead byte, which must fit before `end`, then its code.
                let len = ONIG_ENCODING_UTF8.mbc_enc_len(&input[i..]);
                if len == 1 || i + len > end {
                    return;
                }
                let bytes = &input[i..i + len];
                let code = ONIG_ENCODING_UTF8.mbc_to_code(bytes, len);
                if let Some(member_of) = folds.class_member(code) {
                    letter[0] = member_of;
                    (&letter, len)
                } else if let Some(text) = folds.ligature(bytes) {
                    (text, len)
                } else {
                    return;
                }
            };
            // A literal ending inside a ligature cannot match; its node is
            // passed without a report.
            for &symbol in symbols {
                match self.child(node, symbol) {
                    Some(next) => node = next,
                    None => return,
                }
            }
            chars.push(FoldedChar {
                start: i,
                end: i + len,
                symbols: symbols.len(),
            });
            i += len;
            if let Some(literal) = self.nodes[node as usize].terminal {
                if folds.accepts(literal as usize, input, &chars) {
                    found(literal, i - pos);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_match() {
        let literals: Vec<&[u8]> = vec![b"foo", b"bar", b"baz"];
        let trie = LiteralTrie::build(&literals, false);

        assert_eq!(trie.find_match(b"foobar", 0, 6), Some(3));
        assert_eq!(trie.find_match(b"foobar", 3, 6), Some(3));
        assert_eq!(trie.find_match(b"bazqux", 0, 6), Some(3));
        assert_eq!(trie.find_match(b"qux", 0, 3), None);
    }

    #[test]
    fn test_prefix_overlap() {
        let literals: Vec<&[u8]> = vec![b"ab", b"abc", b"abcd"];
        let trie = LiteralTrie::build(&literals, false);

        // Should return longest match
        assert_eq!(trie.find_match(b"abcde", 0, 5), Some(4));
        assert_eq!(trie.find_match(b"abce", 0, 4), Some(3));
        assert_eq!(trie.find_match(b"abx", 0, 3), Some(2));
        assert_eq!(trie.find_match(b"axx", 0, 3), None);
    }

    #[test]
    fn test_case_insensitive() {
        let literals: Vec<&[u8]> = vec![b"foo", b"BAR"];
        let trie = LiteralTrie::build(&literals, true);

        assert_eq!(trie.find_match(b"FOO", 0, 3), Some(3));
        assert_eq!(trie.find_match(b"foo", 0, 3), Some(3));
        assert_eq!(trie.find_match(b"Bar", 0, 3), Some(3));
        assert_eq!(trie.find_match(b"bar", 0, 3), Some(3));
    }

    #[test]
    fn test_no_match() {
        let literals: Vec<&[u8]> = vec![b"abc"];
        let trie = LiteralTrie::build(&literals, false);

        assert_eq!(trie.find_match(b"abd", 0, 3), None);
        assert_eq!(trie.find_match(b"", 0, 0), None);
    }

    #[test]
    fn test_offset() {
        let literals: Vec<&[u8]> = vec![b"world"];
        let trie = LiteralTrie::build(&literals, false);

        assert_eq!(trie.find_match(b"hello world", 6, 11), Some(5));
        assert_eq!(trie.find_match(b"hello world", 0, 11), None);
    }

    #[test]
    fn test_many_literals() {
        // Simulate a CSS property-like pattern
        let literals: Vec<&[u8]> = vec![
            b"color",
            b"content",
            b"cursor",
            b"display",
            b"direction",
            b"float",
            b"font",
            b"font-size",
            b"font-weight",
            b"height",
            b"left",
            b"margin",
            b"margin-top",
            b"padding",
            b"position",
            b"right",
            b"top",
            b"width",
            b"z-index",
        ];
        let trie = LiteralTrie::build(&literals, false);

        assert_eq!(trie.find_match(b"z-index", 0, 7), Some(7));
        assert_eq!(trie.find_match(b"font-weight:", 0, 12), Some(11));
        assert_eq!(trie.find_match(b"font-size:", 0, 10), Some(9));
        assert_eq!(trie.find_match(b"font:", 0, 5), Some(4));
        assert_eq!(trie.find_match(b"margin-top;", 0, 11), Some(10));
        assert_eq!(trie.find_match(b"margin;", 0, 7), Some(6));
    }
}
