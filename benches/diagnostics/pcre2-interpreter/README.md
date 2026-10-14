# Local PCRE2 interpreter investigation

I measured a repeatable local improvement in Ferroni's isolated Django domain
regex. Batching failed alternative guards saves 11.6% of matching time on the
57 ordinary upstream fixtures. The implementation is now enabled in the VM.
I accept the measured tradeoff: 2.1% more time for the URL batch and 5.8% more
for o200k. Published Blacksmith data still describe their recorded revision;
these local measurements are not substituted into those tables.

The measured baseline is commit `cf95a8f`, recorded in [context.json](context.json).
Later changes on Main are outside this experiment.

## Findings

The source-backed Django domain pattern compiles to 9,008 Ferroni VM operations
with its case-insensitive flags. Oniguruma-style Unicode case folding expands
the broad character classes into many literal alternatives. The CPU profiles
show repeated VM dispatch through `PushOrJumpExact1` and `PushOrJumpByteSet`.

PCRE2's interpreter handles class repetition inside tight loops, including the
`OP_CLASS` and `OP_XCLASS` paths. That is a useful design reference for reducing
interpreter dispatch. It does not establish that all of PCRE2's advantage has
one cause, or that removing Ferroni's case-fold alternatives would preserve its
semantics. See the [pinned PCRE2 10.46 interpreter](https://github.com/PCRE2Project/pcre2/blob/pcre2-10.46/src/pcre2_match.c)
and [PCRE2's performance documentation](https://www.pcre.org/current/doc/html/pcre2perform.html).

The guarded implementation preserves the original bytecode and tests. It batches
at most 1,024 failed forward guards without changing input or stack state. It
retains the existing rules for check-dependent retry counts and falls back at
retry-limit boundaries and with time limits. A differential regression covers
captures, Unicode, malformed bytes, greedy backtracking, FIND_LONGEST, and both
match and search retry limits. It caught a real error-precedence problem in an
earlier revision; the retained candidate fixes it.

## Local results

The tables show median time per **complete fixture batch**, in microseconds.
Changes are median paired after/before ratios from three fresh processes,
with reversed ordering in the second repetition. They are engine regex steps,
not timings for the full upstream application.

| Original workload | Inputs | Baseline | Candidate | Change in matching time |
| --- | ---: | ---: | ---: | ---: |
| Django email domain | 58 | 375.7 | 344.0 | −8.5% |
| Django URL regex | 195 | 2,723.3 | 2,780.5 | +2.1% |
| IPv6 validation | 16 | 4.73 | 4.73 | Inconclusive |
| tiktoken o200k regex | 11 | 120.6 | 127.5 | +5.8% |

The domain improvement and the URL/o200k regressions have the same direction
in every process's confidence interval comparison. The IPv6 intervals overlap.
The full before/after values are in [proof-comparison.json](proof-comparison.json).

I also removed the single 6,403-character domain fixture, which the full Django
email validator would reject through its total-address length precheck. The
remaining 57 fixtures, pattern, flags, and expected results are unchanged. This
checks whether the improvement depends on an oversized engine-only test input.

| Ordinary domain fixtures | Time per 57-input batch |
| --- | ---: |
| Ferroni baseline | 305.5 µs |
| Ferroni candidate | 269.6 µs |
| PCRE2 interpreter | 26.7 µs |

The candidate saves **11.6% of matching time / 13.1% more throughput** in this
subset. Its improvement has non-overlapping interval comparisons in all three
processes. PCRE2 still has a substantial lead here. This candidate does not
close that gap. See [short-comparison.json](short-comparison.json).

Later screening experiments with dispatch tables, eligibility metadata, and
separate VM instantiations did not establish an improvement without losses in
the controls. Only the measured guard-batching implementation is enabled. The next useful
investigation is how to reject impossible case-fold alternatives earlier while
keeping the full Unicode behavior and the ordinary VM inexpensive.

## Measurement and retained evidence

- Apple M1 Ultra, 20 physical cores, 64 GiB RAM; macOS 27.0.1 ARM64.
- Rust 1.99.0, optimized level 3, thin LTO, debug information level 1.
- Thirty Criterion samples per process, one-second warmup, two-second
  measurement window, three independent processes per profile and case.
- Standard public Ferroni API, cached workspace; compilation and Criterion
  analysis are outside matching timings. Results are consumed with `black_box`.
- PCRE2 10.46 **interpreter**, `PCRE2_UTF | PCRE2_UCP`, reused match data and
  context. `PCRE2_NO_UTF_CHECK` skips duplicate validation of valid Rust `&str`
  inputs; it retains Unicode property behavior.
- Source-oracle preflight passes the full Ferroni production/probe corpus
  through both API paths, both tokenizer cases, and all three short-subset
  profiles. The candidate passes `cargo test --release --all-features` and
  Clippy with the deterministic deadline fixture described below.
- Background activity and host power/thermal state are not controlled.
  These are local diagnostics, not new Blacksmith publication results.

[context.json](context.json) records source, corpus, lockfile and binary hashes.
[evidence.tar.gz](evidence.tar.gz) retains the raw Criterion samples and
estimates, process logs, oracle validation, CPU profiles with symbol sidecars,
short-subset harness and catalog, and diagnostic scripts.

[candidate.patch](candidate.patch) records the measured guard-batching change,
now applied to `src/regexec.rs` with standard formatting.
[deadline-fixture.patch](deadline-fixture.patch) fixes a release-mode test race
that I also reproduced on unchanged Main: setting an expired deadline to
`Instant::now()` can still equal the next clock read. A deadline one second in
the past makes the test's intended state deterministic. That fixture fix is
also applied. The retained patches and raw measurements preserve the original
experiment for independent reproduction.

## Isolated measurements and profiles

The existing harness can already select one case and engine. For a diagnostic
comparison with the original fixtures:

```bash
python3 scripts/compare-production.py run /tmp/domain-comparison \
  --diagnostic --cases django_email_domain --engines rust,pcre2 \
  --repetitions 3 --samples 30 --measure-seconds 2 --warmup-seconds 1
```

For CPU profiles, build once with symbols, then run the printed benchmark
executable directly so Cargo compilation is outside the profile:

```bash
PCRE2_SYS_STATIC=1 PCRE2_SYS_DEBUG=1 \
CARGO_PROFILE_BENCH_DEBUG=1 CARGO_PROFILE_BENCH_STRIP=none \
  cargo bench --locked --features ffi --bench production_regex_bench --no-run

PRODUCTION_CASE=django_email_domain PRODUCTION_ENGINE=rust \
  samply record --save-only --unstable-presymbolicate -o /tmp/domain-rust.json.gz \
  /path/to/printed/production_regex_bench --bench --profile-time 12 '/steady/'
```

Set `PRODUCTION_ENGINE=pcre2` for the interpreter counterpart. Select another
catalog case with `PRODUCTION_CASE`, or set `PRODUCTION_CORPUS=tokenizer` for a
tokenizer case. Criterion's `--profile-time` disables statistical analysis;
it is for finding CPU hotspots, not for estimating a speedup. The retained
short-subset harness also demonstrates narrowing the input pool without
changing its source patterns or expected outputs.
