# Same-commit Blacksmith full repeat — October 13, 2026

I repeated all 85 cases on both platforms in [run 38442515829](https://github.com/sebastian-software/ferroni/actions/runs/38442515829). It measures the same merged runtime SHA as the preceding [full run](../blacksmith-2026-10-13-38439201637/README.md). All eight matrix jobs and the summary passed.

The complete suite replaces the original full run in the published figures on both hosts. The [same-commit comparison](same-commit-repeat.md) retains every original/repeated pair, and [all-cases.md](all-cases.md) compares this repeat with October 9. No per-case selection is used.


I verified the merged runtime commit `d791bf1b011f6e98619969872a4d7cb7161912a7` in every clean build receipt, along with the pinned input and lockfile hashes. Every measured executable differs from its pre-CSS equivalent. Compiler and Node versions match the preceding main run; CPU descriptions can differ within the same runner label. In the first full run, the x86 shared job reported an Intel Xeon without AVX-512, replacing an AMD EPYC with AVX-512 in the preceding run. Physical VM placement and scheduling remain uncontrolled. The retained workflow metadata and artifact IDs identify each source run.

The full run covers 85 cases per platform: 32 shared, 17 advanced, 13 micro/compilation and 23 textmate cases. The full repeat covers the same 85 cases on each platform. Validation precedes timing. Unsupported syntax and different-result cases remain excluded, with reasons in `engine-notes.json`.

The workflow requests five seconds, 30 samples and 500 ms warmup. Existing `battle_bench` groups override measurement time to four seconds; captured scanner replays use five seconds, and Shiki JS uses its Node batch timer. Preparation, compilation, input construction and validation stay outside execution timing. Raw Criterion estimates and logs are retained; individual sample files and executables are not uploaded by this workflow, but executable hashes are recorded.

The October 9 baseline combines full run 37989957431 (`a38768bc80121a1bd72e3e280f12063dc407bc04`) for shared/oniguruma/micro with later run 37999607702 (`b93c6d2035f4919480f43d9b7d4134541e4cb9b5`) for textmate. The later CSS baseline already includes pre-filter retirement. There are 81 historical matches per platform; four JSON/Astro cases were added later and carry no historical speedup claim.

Positive percentages mean less execution time. Category changes are equally weighted geometric means of matched case ratios. C-relative changes use `100 × (1 - (new_Ferroni / new_C) / (old_Ferroni / old_C))`; they are diagnostic rather than a causal correction. Separate VMs, sequential engine timings and code layout can affect results. These data do not isolate the effect of an individual optimization, and there is no paired significance test.

See `analysis/integrity.json`, `dispatch.json`, `run.json`, artifact metadata and raw build/measurement receipts. SHA256SUMS seals this report.
