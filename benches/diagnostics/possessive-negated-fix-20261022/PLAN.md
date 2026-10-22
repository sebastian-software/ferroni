# Semver regression investigation and validation plan

Question: can possessive negated byte runs retain their HTML/Jinja savings
without the confirmed Semver capture regression against
`164bb809cea91c478fb5614d5c423922fc150197`?

- Preserve the previous candidate and its evidence before editing.
- Inspect the compiled Semver programs, profile the capture workload, and compare
  generated dispatch code. Do not infer an exact CPU cache or predictor cause
  from timing alone.
- Screen bounded implementation alternatives on the same seven cases, including
  both Semver cases, HTML, Jinja strings/integers, graphemes, and cl100k. The
  capture-only exclusion experiment uses the three directly affected controls.
  Retain every probe, including unsuccessful alternatives.
- Confirm the selected candidate on both Semver cases, HTML, and Jinja strings:
  16 paired repetitions, eight environment sizes with opposite A/B orders,
  30 samples, 0.5 s warmup, 1.5 s measurement per process.
- Run the existing differential tests and full local correctness/lint gates on
  the selected source. Explicit configured stack limits must preserve their
  original outcomes.
- Compare all 29 timed production/tokenizer cases against the previous version
  using the same 16-pair protocol. Repeat a seven-case A/A control using a
  byte-identical copy of the baseline binary (eight pairs; offsets 0, 16, 32,
  and 48 with opposite orders). No builds, tests, profiles, or
  other agent benchmarks may run concurrently with timing.
- Keep all process estimates, samples, oracle results, orders, environment
  offsets, load averages, binary hashes, and the exact candidate patch.
- Report paired medians and descriptive layout-cluster bootstrap intervals,
  both cycle medians, residual uncertainty, and the limits of this interactive
  local machine. No sample or outlier removal; no universal performance claim.

## Follow-up after the first full matrix

The first full matrix (`broad`, binary `fixed001`, identical instruction bytes
to `helper03`) removes the material Semver regression but exposes a repeatable
Jinja integer slowdown: +4.59%, with all 16 pairs slower and both cycle medians
above +4%. Keep that matrix intact and reject this candidate as the final fix.

The next alternative (`compact3`) outlines the entire negated-byte-star arm,
including payload validation, and returns `Option<(run_end, skip_pop)>`. It
retains the same character walker and limit fallback. Screen the same seven
cases with the full 16-pair, 30-sample, 0.5 s / 1.5 s protocol before deciding
whether to run the final 29-case comparison. Use equally long output-directory
names (`probe` and `final`) as an additional control on harness path storage.
This does not imply that a path-length effect has been established.

`compact3` leaves Semver hyphen at +2.05% with a wide interval reaching +6.02%,
and Semver version at +2.21%. Prefer the previously screened, less aggressive
`helper02` alternative: it keeps the byte walker separately out of line. Its
full-protocol seven-case confirmation (`try02`) reports Semver hyphen +0.26%,
Semver version −0.11%, Jinja integer −0.64%, HTML −7.11%, and Jinja strings −4.77%.
The three unaffected controls have intervals inside ±3%. Select `helper02`,
rerun the local gates on its exact source, refresh the A/A control on Semver
hyphen and both Jinja cases (`aaend`), then run all 29 cases again (`final`).
The final matrix uses the identical frozen binary as `try02` and the same
16-pair protocol; no source or benchmark executable changes during timing.

## Follow-up after the second full matrix

The `helper02` matrix (`final`) keeps HTML at −7.00%, Jinja strings at −3.07%,
and Jinja integers at −0.13%. Semver hyphen improves to +1.13%, but its interval
still reaches +6.20%, with cycle medians +1.13% and +2.98%. This does not fully
clear the original caveat. Keep the complete matrix and its analysis in
`helper02-results.json`.

Try retaining the original program-counter advance and executing the existing
pop after a fused scan; with no temporary choices to remove, that pop is a
no-op. The helper returns only the run end. This trades one dispatch for a
smaller change to the shared VM. Validate the differential tests before timing.
The prior focused and full matrices padded to different base sizes (4,224 and
4,288 bytes). Add an explicit environment-base option so subsequent focused
and broad runs can share the same base; retain the original runner and each
run's recorded base and hash. Do not reinterpret earlier runs as identical
layouts or discard their results.

The retained-pop variant (`popleave`, phase `pop01`) fails the full-protocol
seven-case comparison: Semver hyphen +6.94%, HTML −1.85%, and Jinja strings
+0.05%. Reject it. Next, preserve the original backtracking walkers and add a
separate forward-only possessive walker (`stable01`). This removes generic
specialization changes from existing walkers while retaining the outlined
selector and skip-pop behavior. Validate first, then use the same seven-case
16-pair protocol at an explicit 4,288-byte environment base (`sta01`).

`stable01` is rejected: Semver hyphen +6.12%, Semver version +4.10%, and Jinja
integer +4.19%, despite stronger HTML/string gains. Next, `compact4` returns
`Option<(run_end, NonZeroUsize)>` from the whole-opcode helper. The nonzero next
instruction uses the optional result's niche, avoiding a separate failure tag
and preserving a two-word return. Its seven-case phase is `cmp04`, with the
same fixed-base protocol. A proposed separate-walker `compact5` build reused
a stale Cargo artifact because a copied source retained an older mtime; it
has **not** been benchmarked. The stale artifact and logs are labeled in
`stale-build-note.json`, and no timing is attributed to that source.

## Wider layout confirmation of the conservative candidate

If `compact4` does not clear the capture controls, keep `helper02` as the best
candidate before collecting further evidence. Its full matrix has no median
slowdown at or above 3%, but the eight-layout Semver interval remains too wide.
Restore its exact source and verify that a fresh build reproduces the frozen
binary. Do not pool prior screens or the full matrix into this confirmation.

Run a fresh 64-pair Semver A/A control (`aa64x`) at a fixed 4,288-byte base,
using 32 offsets from 0 through 248 bytes in steps of eight, each twice with
opposite orders. Then run the selected candidate on Semver hyphen, Jinja
strings, and Jinja integers with the identical 64-pair protocol (`wide2`).
Use 30 samples, 0.5 s warmup, and 1.5 s measurement throughout. This broadens
the tested address-layout range to four cache-line-sized blocks; it does not
identify a hardware cache mechanism. Keep both cycle medians and all raw
samples. This is additional targeted evidence, not a replacement for the
29-case matrix or its remaining uncertainty.

`compact4` finishes at Semver hyphen +1.44% (interval up to +4.76%), Semver
version +1.73% (up to +3.43%), and Jinja integer +1.17%. It does not clear
both Semver controls and is not selected despite HTML −9.90% and Jinja strings
−5.82%. Restore `helper02` for the preplanned wider confirmation. Leave the
unmeasured `compact5` proposal unselected; its stale artifact is not evidence.

## Completed independent confirmation

The fresh Semver A/A control reports −0.24% (interval −0.94% to +0.04%).
The selected `helper02` comparison reports Semver hyphen −0.32% (−0.67% to
+0.17%; cycle medians −0.33% and −0.32%), Jinja strings −4.82% (−5.18% to
−3.97%; cycles −4.65% and −5.05%), and Jinja integers +0.09% (−0.19% to
+0.36%; cycles −0.01% and +0.19%). Keep this implementation. The full matrix
and all prior results remain separate. Every planned pair completed, and
all 16 measurement phases passed the completeness and provenance audits.
