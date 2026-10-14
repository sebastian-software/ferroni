# Comparator scope and source review

The comparison tests source-backed regex operations with required outputs. A
library's benchmark example is a lead for a workload, not evidence of production
usage or a reason to change the input mix to favor an engine.

## regexr 0.6.1

[regexr](https://github.com/ml-rust/regexr/tree/247a10d35f35fc1900333ca97f497920f784746a)
positions itself as a Rust engine for tokenization and LLM text processing. Its
examples include cl100k_base and o200k_base; its generic email, URL, log and
lookaround examples overlap questions already represented by production cases.
The comparison uses the current original tiktoken definitions, with documented
syntax translation and original Python-regex oracles, rather than assuming the
older expressions in a benchmark are current.

The measured profiles keep default SIMD and explicitly enable `(?u)` for Unicode
word/digit classes and boundaries. Source inspection shows whitespace is already
Unicode-aware, despite the feature guide's ASCII shorthand descriptions. JIT
is off or enabled through `RegexBuilder::jit`; the recorded backend makes any
fallback visible. This compares the library's supported configurations, not a
claim that every JIT-enabled pattern receives generated machine code.

The published crate records source revision
`54a9116abb64b38ae63c687f68489fad57db682f`. The reviewed repository examples
have their own pinned revision in the tokenizer catalog. A source-oracle
segmentation difference does not imply a missing grapheme implementation:
regexr implements GB9c for Indic conjuncts, whereas the retained Symfony oracle
splits the illustrated conjunct. Its matrix cell remains supported; the
specific mismatching task is not timed.

## fancy-regex 0.19.2

[fancy-regex's documented approach](https://github.com/fancy-regex/fancy-regex/blob/4067f51b9adc1a4d05d1bca9d913962d92f91e29/README.md)
combines backtracking for advanced syntax with delegation to Rust regex
subengines. Its Seek prefilter is experimental and disabled by default; the
comparison retains separate seek-off and seek-on profiles. It targets
Oniguruma syntax with the relevant mode enabled, without promising complete
Oniguruma semantics or linear time for every advanced pattern.

Its own benches include parser/compiler costs, pathological backtracking,
lookbehind fixtures and RegexSet scanner examples. These are useful diagnostics.
The source-backed suite already separates compilation, first use and reuse.
TextMate/RegexSet scanners stay in their scanner comparison. The production
[tiktoken Rust core](https://github.com/openai/tiktoken/blob/4e71bbe0c078468e00fefbf94b39849389f346e5/src/lib.rs)
uses fancy-regex, making the separate pre-tokenization corpus a documented
application of this approach. Full BPE tokenization is outside its boundary.
