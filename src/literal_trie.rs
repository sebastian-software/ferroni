/// Compact trie for matching literal alternations.
///
/// When a regex alternation consists entirely of literal strings (e.g.
/// `accent-color|additive-symbols|...|z-index`), we compile them into a trie
/// for O(len) matching instead of O(n*len) backtracking.

pub struct LiteralTrie {
    nodes: Vec<TrieNode>,
    case_insensitive: bool,
    raw_literals: Vec<Vec<u8>>,
}

struct TrieNode {
    /// Sorted (byte, child_index) pairs for binary search lookup.
    children: Vec<(u8, u32)>,
    /// True if a complete literal ends at this node.
    is_terminal: bool,
}

impl LiteralTrie {
    /// Build a trie from a set of literal byte strings.
    /// If `case_insensitive`, all keys are lowercased during insertion
    /// and lookups will also lowercase input bytes.
    pub fn build(literals: &[&[u8]], case_insensitive: bool) -> Self {
        let mut trie = LiteralTrie {
            nodes: vec![TrieNode {
                children: Vec::new(),
                is_terminal: false,
            }],
            case_insensitive,
            raw_literals: literals.iter().map(|l| l.to_vec()).collect(),
        };

        for lit in literals {
            trie.insert(lit);
        }

        trie
    }

    fn insert(&mut self, key: &[u8]) {
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
                        is_terminal: false,
                    });
                    self.nodes[node_idx as usize]
                        .children
                        .insert(pos, (b, new_idx));
                    node_idx = new_idx;
                }
            }
        }
        self.nodes[node_idx as usize].is_terminal = true;
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
    #[inline]
    pub fn find_match(&self, input: &[u8], pos: usize, end: usize) -> Option<usize> {
        let mut node_idx: u32 = 0;
        let mut last_match: Option<usize> = None;
        let mut i = pos;

        // Check if root is terminal (empty string literal — shouldn't happen but be safe)
        if self.nodes[0].is_terminal {
            last_match = Some(0);
        }

        while i < end {
            let b = if self.case_insensitive {
                input[i].to_ascii_lowercase()
            } else {
                input[i]
            };

            let children = &self.nodes[node_idx as usize].children;
            match children.binary_search_by_key(&b, |&(k, _)| k) {
                Ok(child_pos) => {
                    node_idx = children[child_pos].1;
                    i += 1;
                    if self.nodes[node_idx as usize].is_terminal {
                        last_match = Some(i - pos);
                    }
                }
                Err(_) => break,
            }
        }

        last_match
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
