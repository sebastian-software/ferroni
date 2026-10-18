from pathlib import Path
import json, hashlib, gzip, math, shutil, subprocess
out=Path('/private/tmp/ferroni-dfa-homepage-guard-20261018')
repo=Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
data=json.loads((out/'result.json').read_text())
assert (out/'exit-code.txt').read_text().strip()=='0'
assert len(data['runs'])==32 and len(data['cold'])==60
assert data['harnessSha256']==json.loads((out/'plan.json').read_text())['harness_sha256']
assert data['node']=='v24.21.0'
assert data['options']['warmups']=='12' and data['options']['rounds']=='60'
summary=data['summary']['candidate']
assert len(summary['rows'])==14
assert data['engines'][0]['facadeSha256']==data['engines'][1]['facadeSha256']
def median(xs):
    xs=sorted(xs); n=len(xs)
    return xs[n//2] if n%2 else (xs[n//2-1]+xs[n//2])/2

def interval(ratios):
    state=20261017; estimates=[]
    for sample in range(10000):
        selected=[]
        for i in range(len(ratios)):
            state=(state*1664525+1013904223)&0xffffffff
            selected.append(ratios[math.floor(state/(2**32)*len(ratios))])
        estimates.append(median(selected)-1)
    estimates.sort()
    return [estimates[249],estimates[9749]]
rows=[]
for i,row in enumerate(summary['rows']):
    ratios=[]
    for run in data['runs']:
        case=run['cases'][i]
        assert len(case['samplesMs']['baseline'])==60 and len(case['samplesMs']['candidate'])==60
        assert case['sha256']==data['runs'][0]['cases'][i]['sha256']
        assert case['outputSha256']==data['runs'][0]['cases'][i]['outputSha256']
        ratios.append(median(case['samplesMs']['candidate'])/median(case['samplesMs']['baseline']))
    assert ratios==row['ratios']
    assert median(ratios)-1==row['change']
    assert interval(ratios)==row['interval95']
    rows.append({'path':row['path'],'runtime_change_pct':100*row['change'],'interval95_pct':[100*v for v in row['interval95']]})
cold_ratios=[pair['candidate']/pair['baseline'] for pair in data['cold']]
assert cold_ratios==summary['coldRatios']
assert median(cold_ratios)-1==summary['coldChange']
assert interval(cold_ratios)==summary['coldInterval95']
passed=all(row['interval95'][1]<=.02 for row in summary['rows']) and summary['coldInterval95'][1]<=.02
worst=max(rows,key=lambda row:row['interval95_pct'][1])
result={'decision':'merge' if passed else 'close','warm_total_change_pct':100*summary['totalChange'],'worst_warm_upper95_pct':worst['interval95_pct'][1],'worst_warm_path':worst['path'],'cold_change_pct':100*summary['coldChange'],'cold_interval95_pct':[100*x for x in summary['coldInterval95']],'rows':rows}
(out/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
ready=out/'ready-report'; ready.mkdir()
for name in ['plan.json','driver-context.json','summary.json','homepage-harness.mjs','run.py','prepare-report.py']:
    shutil.copy2(out/name,ready/name)
for name in ['result.json','stdout.json','run.log']:
    (ready/(name+'.gz')).write_bytes(gzip.compress((out/name).read_bytes(),mtime=0))
for variant in ['baseline','candidate']:
    shutil.copy2(Path('/private/tmp/ferroni-dfa-replay-20261017/consumer')/(variant+'-build.json'),ready/(variant+'-build.json'))
decision=("I am keeping the small runtime change. The confirmed scanner benefit is useful, the implementation adds only a local bound and no allocation or abstraction, and the current-main homepage guard passes the 2% limit." if passed else "I am closing this experiment. The scanner hotspot benefit is real, but the complete homepage guard does not establish the 2% limit. The full Ferriki gains remain below 1%, so I am not adding further boundary logic on this evidence.")
readme=f'''# Current-main homepage guard for #415

{decision}

This October 18, 2026 decision supersedes the earlier draft recommendation in the parent and scanner reports. I accept a useful local gain from a small implementation change; a 3% reduction in a complete Ferriki task is not a prerequisite for this decision. The 2% regression limit still applies separately to every homepage document and cold startup.

## Fixed comparison

- Ferriki: `e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8`, after Ferriki #279 and #281.
- Ferroni baseline: `92c2363ed48c99f281805b6cfa4278126588a4c5`.
- Ferroni runtime candidate: `adb31da5001459a10d5456209823dbeb8bff38ae`. Later commits change evidence only.
- Apple M1 Ultra, macOS arm64, Node 24.21.0, rustc 1.99.0. Ordinary Ferriki fat-LTO release profile, one codegen unit, no profiling flags or instrumentation.
- The independent baseline/candidate packages and sidecars are the exact current-consumer builds retained in the scanner report. The harness verifies addon hashes, identical Ferriki source, lockfile, compiler, features, and flags. Facade hashes are identical too.
- Frozen 14-document homepage corpus: `aea84dc7b911c79a86dd3631c698c991c36184b8`.
- Before the first timed process I fixed 32 independent warm workers, 12 warm-ups, 60 rotating paired rounds, and 60 alternating cold process pairs. Every process is retained; no cohort is extended.

Warm timing calls the public Node `codeToHtml` API. Every worker first checks exact HTML against Shiki/WASM. Small calls are batched. Cold timing includes imports, corpus reading, grammar loading, and first highlights, excluding process creation. The driver checkout is `ec10cb9`; its four imported helpers are byte-identical to the current consumer revision, as checked before execution. The measured engines are independent saved packages, not that driver's addon.

## Results

Negative values mean less runtime. The estimate is the median independent paired ratio. The paired 95% percentile interval uses 10,000 deterministic bootstrap resamples (seed 20261017) of worker ratios or cold process pairs. These are local per-case intervals, not simultaneous bounds or portable guarantees.

| Homepage document | Runtime change | Paired 95% interval |
| --- | ---: | ---: |
'''
for row in rows:
    readme+=f"| `{row['path']}` | {row['runtime_change_pct']:+.2f}% | {row['interval95_pct'][0]:+.2f}% to {row['interval95_pct'][1]:+.2f}% |\n"
readme+=f'''
The warm total changes by {result['warm_total_change_pct']:+.2f}%. The largest individual warm upper endpoint is {result['worst_warm_upper95_pct']:+.2f}% (`{result['worst_warm_path']}`). Cold startup changes by {result['cold_change_pct']:+.2f}% ({result['cold_interval95_pct'][0]:+.2f}% to {result['cold_interval95_pct'][1]:+.2f}%). The predeclared guard **{'passes' if passed else 'does not pass'}**.

YAML is the one case with a small positive interval: +0.29% runtime (+0.07% to +0.45%). That is a measured local slowdown within the 2% tolerance; the guard does not mean every case becomes faster.

The earlier scanner evidence remains separate: TSX group 2 takes 3.35% less time (paired 95% interval -3.82% to -2.84%). Full native TSX HTML improves by 0.63% on the example and 0.15% on the large fixture. A separate ordinary-driver diagnostic uses 8.40% fewer instructions per TSX replay; this is not an equivalent runtime gain or a Ferriki-addon instruction claim. The outlined scanner function in the inspected thin-LTO binary shrinks from 2,688 to 2,296 bytes; this is not total binary size.

## Validation and artifacts

The runtime and regression tests are unchanged from the scanner follow-up. The candidate already passed the focused prefilter/self-check suites, Ferriki's mandatory core compatibility gate, and complete curated/TIOBE output parity checks. The validated addon is byte-identical to the measured candidate. All PR CI checks at `ca1bda1` passed before this evidence-only update. This comparison adds exact homepage HTML checks in every warm worker.

`result.json.gz` retains every warm sample, cold process pair, fixture/output hash, environment, and build provenance. `summary.json` is reproduced from those raw samples, including every bootstrap endpoint. Build receipts, driver/corpus fingerprints, the fixed plan, exact harness, and reproduction scripts are included. No unfavorable worker or cold pair is removed. `run.py` retains the original absolute paths and checkout assertions; adapt them to fresh owned paths when reproducing. It installs a temporary copy of the harness beside the repository helpers and removes only that exact file afterward. The driver checkout remains clean.

Verify this directory with `shasum -a 256 -c SHA256SUMS`.
'''
(ready/'README.md').write_text(readme)
manifest=''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in sorted(ready.iterdir()) if p.is_file() and p.name!='SHA256SUMS')
(ready/'SHA256SUMS').write_text(manifest)
print(json.dumps(result,indent=2))
