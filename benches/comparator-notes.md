# Comparator scope and source review

The comparison tests source-backed regex operations with required outputs. A
library's benchmark example is a lead for a workload, not evidence of production
usage or a reason to change the input mix to favor an engine.

## regexr (removed)

regexr 0.6.1 was compared in one Blacksmith run
([38571723690](https://github.com/sebastian-software/ferroni/actions/runs/38571723690),
measured at `7b9933c`) with JIT off and on, Unicode mode and default SIMD. It
returned the required output for 10 of the 21 production tasks in the
aggregate and for one of the two tokenizer cases, and its public API has no context-preserving offset
search. On the common production cases it took about 7 times Ferroni's time for
steady reuse and about 4 times for compilation on both platforms; on the
`o200k_base` tokenizer case it took about 17% less time. JIT made no material
difference. It is also far less widely used than the other comparators. With
neither speed, adoption nor coverage to inform a choice, it was removed from
the comparison. The tokenizer corpus keeps its regexr provenance: its inputs
were selected from regexr's tokenizer examples.

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
