# Why Semver regressed

The regression comes from generated-code layout in the shared matcher, rather
than extra matching work in the Semver expression. The selected implementation
keeps possessive-run selection and the byte walker in separate out-of-line
helpers. It gives up some HTML savings to avoid a second capture regression.
The final 29-case results belong in [README.md](README.md); the experiments below
explain the choice and remain separate from that final comparison.

## Evidence and limits of the diagnosis

1. A temporary compilation test dumps both Semver catalog programs. Neither
   contains `CClassNotStar`. The hyphen-range program does have two **positive**
   `CClassMixStar` instructions followed by possessive pops. It cannot execute
   the new negated-byte fusion branch. The test and output are retained as
   `opcode-census.rs` and `opcodes.log`.
2. Separate five-second `sample` profiles locate the Semver workload in
   `match_at_impl::<true, false>`. The VM itself accounts for roughly half the
   samples in this diagnostic loop. These are sampling profiles, not CPU
   instruction or cache-miss counters.
3. Disassembly shows that the original in-dispatch fusion added 26 instructions
   to this VM instance and changed register allocation and placement. A control
   disables fusion at compile time in the capture-tracking instance. That
   restores its original 8,293 instructions and 1,602 SP-relative memory
   instructions. Remaining line-by-line differences are address/relocation
   operands and renamed equivalent helper symbols. The Semver slowdown remains
   in that control's focused timing, so removing the extra branch is insufficient:
   placement matters even when the executed instruction sequence is restored.
4. Outlining selection reduces the median Semver slowdown. Inlining the byte
   walker into this selector recovers more HTML savings, but the full matrix
   exposes a repeatable Jinja integer regression. Keeping the walker separately
   out of line avoids this second regression. Its full matrix still leaves a
   material Semver slowdown unresolved by the descriptive interval, motivating
   the independent wider-layout confirmation recorded in the final report.

Static counts describe disassembled bodies, not instructions executed per match.
The selected capture VM has 8,297 instructions and 1,604 SP-relative memory
instructions, with the same local stack-frame size as the baseline. The total
instruction section grows by 3,840 bytes (0.098%); the `__TEXT` segment stays the
same size. The faster HTML variant has a slightly smaller instruction section,
so code size alone does not explain the result.

The precise instruction-cache, branch-predictor, or address-placement interaction
was not isolated. The diagnosis concerns generated code on this toolchain and
machine; it does not establish a specific hardware mechanism or a portable
performance guarantee.

## Screening alternatives

These short screens use four pairs, two environment offsets with opposite
orders, 20 samples, 0.3 s warmup, and 0.8 s measurement. Negative elapsed-time
changes are faster. They are not pooled into the final comparison.

| Variant                                               | Semver hyphen | HTML markup | Jinja strings | Decision                      |
| ----------------------------------------------------- | ------------: | ----------: | ------------: | ----------------------------- |
| `helper01`: outlined selection, mutable PC reference  |        +7.73% |      −2.50% |        −1.35% | Reject                        |
| `helper02`: return run end and next PC by value       |        −0.12% |      −6.85% |        −3.89% | Retain for long confirmation  |
| `helper03`: inline the byte walker into that selector |        +1.26% |     −10.16% |        −6.27% | Confirm, then measure broadly |
| `track001`: disable fusion in the capture-tracking VM |        +7.37% |     −10.96% |        −2.53% | Reject                        |

The first three screens include both Semver cases, HTML, Jinja strings/integers,
Symfony graphemes, and cl100k. The capture-exclusion control uses Semver hyphen,
HTML, and Jinja strings. Every measured alternative and all raw results remain
in the evidence archive.

## Why the first outlined candidate was rejected

The four-case, 16-pair confirmation of `helper03` reports Semver hyphen +0.68%,
HTML −10.67%, and Jinja strings −7.63%. However, its **full 29-case matrix**
(`broad`, frozen binary `fixed001`, identical instruction bytes to `helper03`)
shows Jinja integer **+4.59%**, with all 16 pairs slower and both cycle medians
above +4%. That matrix also records Semver hyphen +0.45%, HTML −10.48%, and
Jinja strings −6.35%. The positive focused result did not clear this candidate.

`compact3` then outlines the entire opcode, including payload validation, and
returns `Option<(run_end, skip_pop)>`. Its capture VM is smaller than baseline,
but its full-protocol seven-case confirmation leaves Semver hyphen at +2.05%
(interval −0.11% to +6.02%; cycle medians +4.16% and +0.38%) and Semver version
at +2.21%. This remaining uncertainty makes it a weaker choice.

## Selected implementation

The conservative `helper02` confirmation uses 16 pairs, eight offsets with
opposite orders, 30 samples, 0.5 s warmup, and 1.5 s measurement:

| Case           | Median time change | Descriptive 95% interval | Two cycle medians |
| -------------- | -----------------: | -----------------------: | ----------------: |
| Semver hyphen  |             +0.26% |         −0.45% to +1.19% |    +1.68%, −0.75% |
| Semver version |             −0.11% |         −1.48% to +0.24% |    −0.84%, +0.39% |
| HTML markup    |             −7.11% |         −7.70% to −6.86% |    −7.04%, −7.41% |
| Jinja strings  |             −4.77% |         −4.86% to −2.88% |    −5.00%, −3.72% |
| Jinja integers |             −0.64% |         −0.86% to +0.21% |    −0.99%, −0.30% |

The grapheme and cl100k controls remain within ±3% descriptively. The selected
source rebuilds to the **same complete binary hash**, not just identical code
bytes, as the frozen `helper02` used in this confirmation and the final matrix.

`run_negated_byte_star` remains `inline(never)` and returns the run end and next
instruction by value. The VM arm performs one call. `greedy_not_byte_class_loop`
also stays `inline(never)`; its two constant specializations retain or omit
backtrack entries as before. A configured stack limit preserves both original
instructions and their temporary choices.

No compiler representation, opcode, per-pattern field, allocation, dependency,
unsafe code, capture behavior, retry accounting, or encoding rule is added or
changed by this outlining fix. The original fusion's work elimination and its
hard-stack-cap implications remain governed by ADR-017.

## Additional checks after the selected candidate's full matrix

The full `helper02` matrix (`final`) reports Semver hyphen +1.13%, with a
descriptive interval of +0.58% to +6.20%. Its two cycle medians are +1.13% and
+2.98%. HTML remains −7.00%, Jinja strings −3.07%, and Jinja integers −0.13%.
This improves the original regression but does not by itself clear the
remaining interval-based caveat. The focused confirmation used a 4,224-byte
environment base; the full matrix used 4,288 bytes. Subsequent focused runs
fix that base explicitly instead of assuming the layouts are identical.

Three further alternatives use the same seven-case, 16-pair protocol at the
4,288-byte base:

| Variant                                                          | Semver hyphen | Semver version | HTML markup | Jinja strings | Jinja integers |
| ---------------------------------------------------------------- | ------------: | -------------: | ----------: | ------------: | -------------: |
| `popleave`: retain the empty pop and return only the run end     |        +6.94% |         +2.54% |      −1.85% |        +0.05% |         +0.23% |
| `stable01`: preserve existing walkers, add a forward-only walker |        +6.12% |         +4.10% |     −12.46% |        −7.22% |         +4.19% |
| `compact4`: whole-opcode helper, optional nonzero next PC        |        +1.44% |         +1.73% |      −9.90% |        −5.82% |         +1.17% |

None supersedes `helper02`. The last variant reduces the tracked VM to 8,266
instructions, 1,576 SP-relative memory instructions, and a 0x3f0 local frame.
Its `Option<(usize, NonZeroUsize)>` fits the run end and next instruction into
two words without a separate failure tag. Nevertheless, both Semver intervals
still reach past +3%, and the integer case is slower in 15 of 16 pairs. Static
code reduction is useful evidence, not a substitute for timing.

An unmeasured proposal, `compact5`, changes that variant's walker back to
`inline(never)`. Its initial source copy preserved an old modification time,
so Cargo reused `compact4`'s artifact. It was caught by identical binary hashes
and the missing compilation step, before any timing. The intended source,
stale logs, and `stale-build-note.json` remain available; the `compact5` entries
in `codegen-compact45` describe that stale artifact and are not evidence for
the proposed source. Every measured candidate's build log contains an actual
compilation step. Restoring the selected source uses a fresh modification time
and reproduces the complete frozen `helper02` binary exactly.

The recorded plan now confirms the selected binary independently over 32
environment offsets, each with opposite orders, using 64 pairs for Semver
hyphen and both Jinja cases. The fresh A/A control uses the same layout range.
Those results are reported separately from the full matrix and all screens.

The completed wider confirmation supports the selected implementation: Semver
hyphen −0.32% (interval −0.67% to +0.17%), Jinja strings −4.82% (−5.18% to
−3.97%), and Jinja integers +0.09% (−0.19% to +0.36%). Both Semver cycle
medians are near zero, and both string cycles exceed the 3% target. The fresh
Semver A/A control is −0.24%. These results remain separate from the smaller
full matrix, whose Semver interval still reaches +6.20%.
