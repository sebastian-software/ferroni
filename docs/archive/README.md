# Archive

Archived research spikes, profiling harnesses and PR journals. Nothing in this
directory is built by cargo: `examples/` is reserved for user-facing examples,
and these files are experiment records, not API demonstrations. To run a
research harness, copy its `.rs` file into `examples/` and leave the rest here:
the look-behind harness loads its Node helper from `research/`. Each file names
its own command.

## Research

| File                                                                                                   | What it recorded                                                |
| ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| [`research/research_ecmascript_lookbehind_NOTES.md`](research/research_ecmascript_lookbehind_NOTES.md) | Findings of the issue #44 ECMAScript look-behind spike          |
| [`research/research_ecmascript_lookbehind.rs`](research/research_ecmascript_lookbehind.rs)             | Differential driver: same patterns through Ferroni and Node     |
| [`research/research-ecmascript-lookbehind-node.js`](research/research-ecmascript-lookbehind-node.js)   | Node helper for that differential                               |
| [`research/profile_named_capture.rs`](research/profile_named_capture.rs)                               | Hot-loop profiling of named-capture overhead vs. a bare pattern |

## Experiments

| File                                                                                     | What it recorded                                                               |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [`experiments/197-backtracking-optimizer.md`](experiments/197-backtracking-optimizer.md) | Journal of the PR for issue #197, the opt-in decimal backtracking optimization |

These records describe the state of the engine at the time they were run. Treat
their numbers and conclusions as dated evidence, not as current behavior.
