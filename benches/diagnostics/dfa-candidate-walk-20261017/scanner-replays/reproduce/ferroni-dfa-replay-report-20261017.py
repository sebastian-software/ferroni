import gzip
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import tarfile

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    validations = json.loads((OUT/'validation-retry/summary.json').read_text())
    assert len(validations) == 3 and all(r['state'] == 'PASSED' for r in validations)
    first = json.loads((OUT/'validation/summary.json').read_text())
    assert all(r['state'] == 'PASSED' for r in first[:2])
    spec = importlib.util.spec_from_file_location('paired', '/private/tmp/ferroni-dfa-replay-run-20261017.py')
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    records = json.loads((OUT/'measurements.json').read_text())
    assert len(records) == 128
    scanner = json.loads((OUT/'summary.json').read_text())
    assert runner.summarize(records) == scanner
    for record in records:
        raw = OUT / record['estimate']['raw_directory']
        assert json.loads((raw/'benchmark.json').read_text())['full_id'] == record['benchmark']
        assert json.loads((raw/'estimates.json').read_text())['mean']['point_estimate'] == record['estimate']['ns']
        assert len(json.loads((raw/'sample.json').read_text())['iters']) == 50
    for variant in ['baseline','candidate']:
        receipt = json.loads((OUT/f'{variant}-build.json').read_text())
        assert all(digest(OUT/'binaries'/variant/name) == value for name,value in receipt['executables'].items())
    consumer = json.loads((OUT/'consumer/summary.json').read_text())
    import random
    workers = json.loads((OUT/'consumer/tsx-confirmation.json').read_text())['runs']
    assert len(workers) == 8
    for row in consumer:
        ratios=[]
        for worker in workers:
            item=next(c for c in worker['cases'] if c['id']==row['case'])
            ratios.append(statistics.median(item['samplesMs']['candidate'])/statistics.median(item['samplesMs']['baseline']))
        rng=random.Random(41520261017)
        boot=sorted(statistics.median(rng.choices(ratios,k=len(ratios))) for _ in range(10000))
        assert (statistics.median(ratios)-1)*100 == row['runtime_change_pct']
        assert (boot[249]-1)*100 == row['lower95_pct'] and (boot[9749]-1)*100 == row['upper95_pct']
    assert all(r['runtime_change_pct'] > -3 for r in consumer)
    profiles = {}
    for language in ['cpp','tsx']:
        profiles[language] = {}
        for variant in ['baseline','candidate']:
            data = json.loads((OUT/f'profiles/{language}-{variant}.json.gz.analysis.json').read_text())
            profiles[language][variant] = {'samples':data['samples'], 'dfa_candidates_inclusive_cpu_pct':data['groups']['dfaCandidates']['percent'], 'top_self':data['self'][:8]}
    summary = {'scanner':scanner,'consumer':consumer,'instructions':json.loads((OUT/'instructions/summary.json').read_text()),
               'counters':json.loads((OUT/'diagnostics/summary.json').read_text()),'profiles':profiles,
               'validation':first+validations,'decision':'Keep #415 as a draft; no confirmed 3% full-Ferriki HTML gain. The conditional new homepage guard was not run.'}
    stage = OUT/'ready-report'
    stage.mkdir()
    (stage/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    for name in ['plan.json','build-context.json','baseline-build.json','candidate-build.json']:
        shutil.copy2(OUT/name,stage/name)
    shutil.copy2(OUT/'consumer/plan.json',stage/'consumer-plan.json')
    patch = subprocess.check_output(['git','-C',str(ROOT),'diff','92c2363ed48c99f281805b6cfa4278126588a4c5','adb31da5001459a10d5456209823dbeb8bff38ae','--','src/dfa_prefilter.rs','tests/scanner_prefilter_regressions.rs'])
    (stage/'runtime-and-tests.patch').write_bytes(patch)
    scripts = stage/'reproduce'
    scripts.mkdir()
    for name in ['build','run','diagnostics','instructions','profile','validation','validation-retry','report']:
        source=Path(f'/private/tmp/ferroni-dfa-replay-{name}-20261017.py')
        shutil.copy2(source,scripts/source.name)
    for source in [Path('/private/tmp/ferroni-dfa-current-consumer-build-20261017.py'), Path('/private/tmp/ferriki-profile-after-20261017/analyze-profile.mjs'), Path('/private/tmp/ferriki-independent-perf-20261017/save-package.py')]:
        shutil.copy2(source,scripts/source.name)
    files=[]
    for directory in ['raw','diagnostics','instructions','profiles','assembly','validation','validation-retry']:
        files.extend(p for p in (OUT/directory).rglob('*') if p.is_file())
    files.extend(p for p in OUT.iterdir() if p.is_file())
    files.extend(p for p in (OUT/'consumer').iterdir() if p.is_file() and p.suffix in ['.json','.log','.lock'])
    files=sorted(set(files))
    manifest=''.join(f'{digest(p)}  {p.relative_to(OUT).as_posix()}\n' for p in files)
    with (stage/'raw-results.tar.gz').open('wb') as binary:
        with gzip.GzipFile(fileobj=binary,mode='wb',mtime=0) as compressed:
            with tarfile.open(fileobj=compressed,mode='w') as archive:
                for path in files:
                    info=archive.gettarinfo(str(path),arcname=path.relative_to(OUT).as_posix())
                    info.uid=info.gid=0;info.uname=info.gname='';info.mtime=0
                    with path.open('rb') as data:archive.addfile(info,data)
                contents=manifest.encode()
                info=tarfile.TarInfo('RAW_SHA256SUMS');info.size=len(contents);info.mtime=0
                archive.addfile(info,io.BytesIO(contents))
    with tarfile.open(stage/'raw-results.tar.gz','r:gz') as archive:
        for line in archive.extractfile('RAW_SHA256SUMS').read().decode().splitlines():
            expected,name=line.split('  ',1)
            assert hashlib.sha256(archive.extractfile(name).read()).hexdigest()==expected
    table='\n'.join(f'| `{r["case"]}` | {r["runtime_change_pct"]:+.2f}% | {r["lower95_pct"]:+.2f}% to {r["upper95_pct"]:+.2f}% |' for r in scanner)
    native='\n'.join(f'| `{r["case"]}` | {r["runtime_change_pct"]:+.2f}% | {r["lower95_pct"]:+.2f}% to {r["upper95_pct"]:+.2f}% |' for r in consumer)
    text=f'''# Scanner replay follow-up for #415

I found a repeatable local benefit in a TSX injection scanner, but the change still does not meet the 3% full-Ferriki latency target. I am keeping the PR as a draft.

The isolated TSX group 2 replay takes 3.35% less time at the central estimate. Complete TSX scanner replay uses about 8.40% fewer retired instructions in a separate diagnostic. In current Ferriki, however, full native HTML takes only 0.63% less time on the TSX example and 0.15% less time on the large TSX fixture. The large fixture's interval includes a small slowdown.

## Revisions and environment

- Ferroni baseline: `92c2363ed48c99f281805b6cfa4278126588a4c5` (2.2.2).
- Ferroni candidate: `adb31da5001459a10d5456209823dbeb8bff38ae`, the original runtime change in this PR. This follow-up changes evidence only.
- Ferriki consumer: `e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8`, after Ferriki #279 and #281 were merged.
- Apple M1 Ultra, macOS arm64, rustc 1.99.0, LLVM 23.1.1, Node 24.21.0, pnpm 10.28.2.
- Scanner binaries use the ordinary thin-LTO bench/release profiles. Ferriki addons use its ordinary fat-LTO release profile with one codegen unit. No CPU-specific flags, instrumentation, or alternative allocator are used for latency measurements.
- Each pair is built sequentially at the same source and target paths, with the same compiler, features, and lockfile. Immutable binaries and build receipts are retained locally; their hashes are in the receipts here.

These are local diagnostic results. They do not replace the published Blacksmith comparisons or establish portability to another CPU or toolchain.

## Real scanner traces

The checked-in C++ trace contains 4,862 calls, 96 scanner instances, and 1,621 subject identities. TSX contains 23,088 calls, 48 scanner instances, and 784 identities. The existing hot-group selection is retained. Both replay and Ferriki's tokenizer call `find_next_match_utf16`.

Every timed process first validates all captured matches and captures, cached and uncached scanners, hot groups, and the independent C Oniguruma oracle. Compilation, JSON parsing, validation, and fresh string construction are outside Criterion's timer. Returned scanner results are consumed. Scanner state persists across replays; subject wrappers are fresh.

I fixed eight baseline/candidate process pairs for all eight cases before the first timed process. Engine order alternates, and case order rotates. Each process uses 1 second of warm-up, 3 seconds of measurement, and 50 Criterion samples. All 128 processes and their raw samples are retained. No processes are discarded and no confirmation cohort is added.

Negative changes mean less time. The central estimate is the median paired process-mean ratio. The interval is a deterministic 10,000-resample percentile bootstrap over the eight process pairs, not over individual Criterion samples.

| Case | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
{table}

The prefilter-off documents are controls for layout and environmental variation. They also fluctuate: individual TSX control pairs range from -10.59% to +6.25%. Background backup processes were observed during the run. This does not identify the cause of any individual outlier. The full C++ estimate is especially uncertain. The hot-group times are independent experiments and cannot be added to reconstruct document time.

## Current Ferriki confirmation

The confirmed TSX hotspot justified a separate consumer check. Its plan was fixed before the first consumer timing. Both addons contain identical current Ferriki source and the same patched lockfile, differing only in the Ferroni runtime change. Their package snapshots contain independent sidecar copies.

The existing paired native-boundary harness runs eight independent processes, 12 warm-ups, and 60 rotating paired rounds over the two curated TSX fixtures. It checks exact output equality before timing. The boundary includes tokenization and complete native inline HTML rendering; it excludes the unchanged Node facade. The intervals below resample independent worker ratios.

| Case | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
{native}

Neither target case reaches a 3% reduction. Under the fixed decision rule I therefore did not run the conditional new homepage regression guard. The earlier homepage experiment remains in the parent report, including its unresolved +2% bound; these new results do not establish that bound on current main.

## Why the hotspot benefit is real but limited

Separate temporary counters are retained as a patch in the archive. They are never compiled into a timed candidate. All captured results, including C parity, pass before the counter run. Counters are reset after five warm replays, then normalized by completed replays.

- Complete TSX visits the changed first-phase limit site about 1.21 million times per replay, compared with about 40,675 for C++. About 97.3% of TSX's walked bytes are in that first phase.
- TSX group 2 walks 454,880 bytes per replay, entirely in the first phase. It never enters the long-pattern admission phase in this diagnostic. Its gain therefore does not depend on the second-phase boundary change.
- The ordinary C++ benchmark's outlined `scan_candidates` function shrinks from 2,688 to 2,296 bytes on this toolchain. The retained ARM assembly shows the boundary consolidation. This is one function, not total binary size or a speed measurement.
- Warm CPU samples put the baseline candidate-search subtree at 13.1% for C++ and 58.9% for TSX. The main C++ regex VM leaf alone accounts for about half its samples. Profiles include driver bookkeeping and string construction and are diagnostic, not latency ratios.

I also measured retired instructions with the ordinary bounded replay driver. Each of two balanced repetitions runs a 1-second process and a 5-second process, subtracts their total instruction counts, and divides by the difference in completed replays. This removes repeated initialization and validation approximately. It includes string construction, timer calls, and timing-vector bookkeeping, which Criterion excludes. The two repetitions give about **-1.03% instructions per C++ replay** and **-8.40% per TSX replay**. These instruction savings do not imply the same runtime reduction, and do not describe Ferriki's fat-LTO addon.

## Validation

- `cargo test --offline --locked --features prefilter-self-check --test scanner_prefilter_regressions --test compat_prefilter_differential`: passed.
- `cargo fmt -- --check`: passed.
- Current Ferriki's mandatory `pnpm run test:ferriki-compat:core`: passed with the candidate dependency.
- Complete `check:bench-curated` and `check:bench-tiobe` output parity gates: passed.
- The addon validated by those gates is byte-identical to the measured candidate.
- Every retained Criterion estimate is checked against its raw JSON. The paired scanner summaries are reproduced exactly, and every archived member is verified against its SHA-256 manifest.

The first consumer gate stopped in dependency preflight: a build-only Cargo patch was not visible to `cargo tree --locked`. I repeated the gate with the same local patch in temporary `.cargo/config.toml`. Both attempts are retained. The patch configuration and original lockfile were restored afterward; no production source or dependency pin changed.

## Artifacts and reproduction

`summary.json` collects all derived results. `plan.json` and `consumer-plan.json` preserve the two predeclared latency plans. `raw-results.tar.gz` contains all Criterion samples and process logs, build receipts/logs, instruction-counter runs, four warmed CPU profiles and their symbols, counter instrumentation and output, assembly, consumer samples, and both validation attempts. Its `RAW_SHA256SUMS` verifies every member. The outer `SHA256SUMS` verifies this report and its artifacts.

The exact run scripts are under `reproduce/`. They retain their original absolute paths and worktree safety assertions. For another checkout, use the pinned revisions above and adapt those paths and assertions to fresh owned worktrees before running them. `profile-cpp-scanner` and Criterion boundaries are documented in `benches/cpp_scanner/README.md`; the TSX fixture is documented in `benches/tsx_scanner/README.md`. The consumer uses the unchanged `node/scripts/bench-native-boundary-paired.mjs` at the pinned Ferriki revision.

The diagnostic patch adds counters and temporary exports only for that diagnostic build. The ordinary binaries are built and copied before instrumentation, and production source is restored before profiling and consumer builds. Do not use counter-build timings as performance evidence.
'''
    (stage/'README.md').write_text(text)
    manifest=''.join(f'{digest(p)}  {p.relative_to(stage).as_posix()}\n' for p in sorted(stage.rglob('*')) if p.is_file())
    (stage/'SHA256SUMS').write_text(manifest)
    for line in manifest.splitlines():
        expected,name=line.split('  ',1)
        assert digest(stage/name)==expected
    print(json.dumps({'stage':str(stage),'raw_files':len(files),'archive_bytes':(stage/'raw-results.tar.gz').stat().st_size,'checksums_verified':True},indent=2))

if __name__ == '__main__':
    main()
