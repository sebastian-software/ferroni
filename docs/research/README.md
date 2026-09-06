# Research notes

Archived research spikes and profiling harnesses. Nothing in this directory is
built by cargo: `examples/` is reserved for user-facing examples, and these
files are experiment records, not API demonstrations. To run one, copy the
`.rs` file into `examples/` and leave the rest here -- the look-behind harness
loads its Node helper from this directory. Each file names its own command.

| File                                                                                 | What it recorded                                                |
| ------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| [`research_ecmascript_lookbehind_NOTES.md`](research_ecmascript_lookbehind_NOTES.md) | Findings of the issue #44 ECMAScript look-behind spike          |
| [`research_ecmascript_lookbehind.rs`](research_ecmascript_lookbehind.rs)             | Differential driver: same patterns through Ferroni and Node     |
| [`research_ecmascript_lookbehind_node.js`](research_ecmascript_lookbehind_node.js)   | Node helper for that differential                               |
| [`profile_named_capture.rs`](profile_named_capture.rs)                               | Hot-loop profiling of named-capture overhead vs. a bare pattern |

These spikes describe the state of the engine at the time they were run. Treat
their numbers and conclusions as dated evidence, not as current behavior.
