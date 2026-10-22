# Local verification plan

Question: does the possessive negated-byte-class optimization retain its gain
across the production/tokenizer cases, and are the previously observed SemVer
regressions repeatable against `164bb809cea91c478fb5614d5c423922fc150197`?

The matrix was fixed before execution. These interpretation rules were recorded
while its first repetition was in progress, before inspecting any version-comparison
results. The runtime source, compiler profile, dependencies, feature set, and
retained executables stay fixed.
The existing candidate's completed correctness checks remain applicable; each
benchmark process also validates its source oracle before timing.

## Matrix

- All 29 executable cases in the production and tokenizer catalogs.
- Standard Rust API, steady reuse only; no claim about compile or first-use cost.
- Sixteen paired processes per case: eight environment-size offsets (0, 8, 16,
  24, 32, 40, 48, 56 bytes), each revisited with opposite binary execution order.
- Shuffled case order in each repetition, with the seed retained.
- One padded environment-size base for the whole comparison, including both
  cycles; equal-length executable paths and two-digit repetition directories.
- Criterion: 30 samples, 0.5 s warmup, 1.5 s measurement, 10,000 resamples.
- Before that matrix, eight A/A pairs on seven selected cases using byte-identical
  copies of the baseline. These include both SemVer cases, Jinja integer/string,
  markup, Markdown emphasis, and cl100k. A/A measures uncontrolled variation;
  its result is not subtracted from the version comparison.
- Every process completes before the next starts. No task builds, tests, or
  profiles run during timing. Other user/system activity remains uncontrolled.
- Each process has a 90-second timeout; a failed process stops the matrix and
  remains visible. No fastest-run selection or outlier removal.

## Interpretation

Report the median paired percentage change, both cycle medians, all per-pair
changes, and a descriptive 95% interval from 10,000 bootstrap resamples grouped
by environment offset. Both opposite-order pairs stay together in a resampled
layout cluster. The interval is not corrected for 29 comparisons or uncontrolled
background activity; do not present it as a portable or simultaneous guarantee.

A repeatable gain at the 3% target requires a median at or below -3%, both cycle
medians at or below -3%, and the interval's upper endpoint below zero. A repeatable
material regression uses the symmetric criteria: median and both cycle medians
at or above +3%, with the lower endpoint above zero. Show smaller and inconclusive
movements explicitly rather than rounding them away. An interval crossing +3%
leaves a material slowdown unresolved even when its point estimate is smaller.

The final recommendation must account for the broad matrix and the A/A control,
not only the previously promising workloads. Preserve all earlier measurements
and runtime changes; this task verifies performance and does not automatically
publish, merge, or retune the optimization.
