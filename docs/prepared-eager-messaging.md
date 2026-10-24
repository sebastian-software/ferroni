# Prepared Eager release messaging

This is the claim and evidence reference for release copy, the README, and the
website. The [user guide](app/routes/guide/precompiled-eager.mdx) owns setup,
compatibility, and fallback instructions. Reviewed on October 24, 2026.

## Audience and promise

For developers of tokenizers, editors, and TextMate runtimes that load the same
scanner patterns repeatedly:

> Prepare scanner groups once. Reuse them across starts.

Ship compiled programs and prepared Eager scanner-group instructions as portable
assets for compatible native and WebAssembly builds. This moves reusable
preparation into the asset-generation step. The application keeps the original
patterns and uses normal Eager preparation for missing or incompatible groups.
The next action is the [producer/consumer guide](app/routes/guide/precompiled-eager.mdx).

## Claims and evidence

| Claim                                                            | Evidence                                                                                                                                                                          | Qualification                                                                                                                                                                            |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reuse compiled programs and prepared Eager groups across starts  | [`PrecompiledEager`](../src/scanner/prepared.rs) and [public API tests](../tests/precompiled_eager.rs)                                                                            | Optional `precompiled-eager` feature; only Eager consumes prepared groups.                                                                                                               |
| Share the same pair between compatible native and Wasm consumers | [Frozen native-produced fixtures](../tests/fixtures/precompiled-eager/README.md), WASI tests, and the [browser conformance example](../examples/precompiled_eager_portability.rs) | Format, feature capability, and compile settings must match. This is portable instruction data, not native machine code.                                                                 |
| Decode only requested records and preserve normal fallback       | [Lazy group loader](../src/scanner/group_prototype.rs) and public API tests                                                                                                       | Opening still checks indexes and the pair binding. Optional full-group preflight adds work and does not retain decoded groups.                                                           |
| Move reusable preparation out of startup                         | Exported group instructions load without rebuilding the dependency automata                                                                                                       | Per-pattern helpers, validation, scratch space, and input-dependent caches remain local. A smaller amount of preparation does not guarantee lower total startup time for every workload. |
| Publish or embed prepared assets                                 | [Runnable producer/consumer example](../examples/precompiled_eager.rs) and [Zstd transport tooling](../scripts/portable-eager-package.mjs)                                        | The host owns grammar traversal, asset hosting, fetching, decompression, and producer trust. Ferroni does not provide a hosted grammar catalog or browser fetch binding.                 |

The [retained TextMate measurements](../benches/diagnostics/textmate-startup-20261024/README.md)
compare the earlier prepared FPRT002 layout with FPRT003, using a frozen prototype
harness. They establish that layout change's startup and size effects; they are
not a new benchmark of this public API against normal Eager. Timing excludes
actual network transfer, file I/O, themes, and HTML rendering. The published
[regex throughput results](app/routes/perf/regex-workloads.mdx) measure a separate
workload and do not prove an archive-loading or search-speed gain.

Do not promise zero setup, that all Eager work is serialized, identical search
timings, a percentage speedup for all grammars, or a security boundary based on
checksums. Matching semantics and timing are different claims.

## Market context

Saving compiled regex data already exists. Primary documentation checked on
October 24, 2026:

- [PCRE2 serialization](https://www.pcre.org/current/doc/html/pcre2serialize.html)
  saves compiled patterns with version, width, and environment restrictions;
  its JIT data cannot be saved and reloaded.
- [Hyperscan serialization](https://intel.github.io/hyperscan/dev-reference/serialization.html)
  supports moving compiled pattern databases between compatible hosts, with
  version and platform checks.
- [regex-automata DFA serialization](https://docs.rs/regex-automata/0.4.18/regex_automata/dfa/index.html#example-deserialize-a-dfa)
  supports serialized dense and sparse DFAs and loading them without compiling
  the patterns again.

These are examples of existing capabilities, not an exhaustive market survey or
performance comparison. They rule out a broad "first regex engine with saved
compilation" claim. They do not establish whether another project offers the
same combination as Ferroni.

Lead with the concrete combination: Oniguruma-compatible scanner behavior,
prepared Eager groups, portable native/Wasm assets, and fallback to the original
patterns. Do not claim "the only engine," "world first," or exclusive ownership
of regex serialization without a separate, scoped comparison.

## Release consistency

- Use `precompiled-eager` for the public feature and `PrecompiledEager` for the
  paired archive API. The old feature name is a compatibility alias.
- Distinguish ordinary `precompiled` programs from prepared Eager group data.
  The feature is not a fourth acceleration mode and does not add a `Regex`
  archive constructor.
- The new feature is not in 2.6.0. Publish installation claims with a release
  that contains it; the release workflow owns version and changelog updates.
- Keep Ferriki integration and any centrally hosted syntax packages out of this
  release's claims. The capability to deliver assets is not such a deployment.
