import hashlib
import json
import math
import os
from pathlib import Path
import random
import statistics
import subprocess
import time

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')
CASES = [
    ('cpp_document', 'cpp_scanner_bench', 'cpp_scanner/document', False),
    ('cpp_group_78', 'cpp_scanner_bench', 'cpp_scanner/group_78', False),
    ('tsx_document', 'ferriki_scanner_bench', 'tsx_scanner/document', False),
    ('tsx_group_2', 'ferriki_scanner_bench', 'tsx_scanner/group_2', False),
    ('tsx_group_1', 'ferriki_scanner_bench', 'tsx_scanner/group_1', False),
    ('tsx_group_29', 'ferriki_scanner_bench', 'tsx_scanner/group_29', False),
    ('cpp_document_prefilter_off', 'cpp_scanner_bench', 'cpp_scanner/document', True),
    ('tsx_document_prefilter_off', 'ferriki_scanner_bench', 'tsx_scanner/document', True),
]
PLAN = {
    'question': 'Does PR #415 reduce the cost of real C++ or TSX scanner replays by at least 3%?',
    'phase': 'fixed confirmation; no screening or optional extension',
    'repetitions': 8, 'cases': CASES, 'samples_per_process': 50,
    'warmup_seconds': 1, 'measurement_seconds': 3,
    'ordering': 'Rotate case order by repetition; alternate baseline/candidate within each case and repetition',
    'primary_cases': ['cpp_document', 'tsx_document'],
    'secondary_cases': 'The four hot groups locate a causal effect; prefilter-off documents are layout/noise controls.',
    'summary': 'Median paired process mean-latency ratio minus one, with a deterministic 10,000-resample paired bootstrap 95% interval over process pairs.',
    'decision': 'A document reduction of at least 3% with its interval wholly below zero warrants a separate confirmation in current Ferriki. A hot-group reduction alone cannot qualify this PR as a public highlighting improvement.',
    'limits': 'Local Apple Silicon, thin-LTO scanner build; no cross-platform or Ferriki end-to-end performance claim.',
    'frozen_at': time.time(),
}

def summarize(records):
    rows = []
    for case, _, _, off in CASES:
        pairs = {}
        for record in records:
            if record['case'] == case:
                pairs.setdefault(record['repetition'], {})[record['variant']] = record['estimate']['ns']
        complete = [p for p in pairs.values() if set(p) == {'baseline', 'candidate'}]
        if not complete:
            continue
        ratios = [p['candidate'] / p['baseline'] for p in complete]
        rng = random.Random(41520261017)
        boot = sorted(statistics.median(rng.choices(ratios, k=len(ratios))) for _ in range(10000))
        rows.append({
            'case': case, 'prefilter_off': off, 'pairs': len(complete),
            'baseline_median_ns': statistics.median(p['baseline'] for p in complete),
            'candidate_median_ns': statistics.median(p['candidate'] for p in complete),
            'runtime_change_pct': (statistics.median(ratios) - 1) * 100,
            'lower95_pct': (boot[249] - 1) * 100,
            'upper95_pct': (boot[9749] - 1) * 100,
            'pair_changes_pct': [(r - 1) * 100 for r in ratios],
        })
    return rows

def estimate(raw, benchmark):
    found = []
    for path in raw.rglob('new/benchmark.json'):
        if json.loads(path.read_text())['full_id'] == benchmark:
            mean = json.loads(path.with_name('estimates.json').read_text())['mean']
            sample = json.loads(path.with_name('sample.json').read_text())
            found.append({'ns': mean['point_estimate'],
                          'lower_ns': mean['confidence_interval']['lower_bound'],
                          'upper_ns': mean['confidence_interval']['upper_bound'],
                          'sampling_mode': sample.get('sampling_mode'),
                          'iterations': sum(sample['iters']), 'samples': len(sample['iters']),
                          'raw_directory': str(path.parent.relative_to(OUT))})
    assert len(found) == 1, (benchmark, found)
    return found[0]

def main():
    assert subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip() == 'adb31da5001459a10d5456209823dbeb8bff38ae'
    assert subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True).strip() == ''
    assert not (OUT / 'plan.json').exists()
    assert (OUT / 'candidate-build.json').exists(), 'Both builds must finish before timing'
    for variant in ['baseline', 'candidate']:
        receipt = json.loads((OUT / f'{variant}-build.json').read_text())
        for name, expected in receipt['executables'].items():
            actual = hashlib.sha256((OUT / 'binaries' / variant / name).read_bytes()).hexdigest()
            assert actual == expected
    (OUT / 'plan.json').write_text(json.dumps(PLAN, indent=2) + '\n')
    records = []
    for rep in range(8):
        order = CASES[rep % len(CASES):] + CASES[:rep % len(CASES)]
        for case, target, benchmark, off in order:
            case_index = next(i for i, c in enumerate(CASES) if c[0] == case)
            variants = ['baseline', 'candidate'] if (rep + case_index) % 2 == 0 else ['candidate', 'baseline']
            for variant in variants:
                raw = OUT / 'raw' / f'pair-{rep + 1:02}' / case / variant
                raw.mkdir(parents=True)
                binary = OUT / 'binaries' / variant / target
                command = [str(binary), '--exact', benchmark, '--bench', '--noplot', '--sample-size', '50', '--measurement-time', '3', '--warm-up-time', '1']
                env = dict(os.environ, SCANNER_REPLAY_ID=benchmark, CRITERION_HOME=str(raw))
                env.pop('FERRONI_BENCH_PREFILTER', None)
                if off:
                    env['FERRONI_BENCH_PREFILTER'] = '0'
                started = time.time()
                with (raw / 'process.log').open('w') as log:
                    subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=120)
                assert 'VALIDATED' in (raw / 'process.log').read_text()
                record = {'case': case, 'benchmark': benchmark, 'variant': variant, 'repetition': rep + 1,
                          'prefilter_off': off, 'command': command, 'started_at': started, 'finished_at': time.time(),
                          'estimate': estimate(raw, benchmark)}
                records.append(record)
                (OUT / 'measurements.json').write_text(json.dumps(records, indent=2) + '\n')
            pair = [r for r in records if r['case'] == case and r['repetition'] == rep + 1]
            by_variant = {r['variant']: r['estimate']['ns'] for r in pair}
            print(f'pair {rep + 1}/8 {case}: {(by_variant["candidate"] / by_variant["baseline"] - 1) * 100:+.2f}%', flush=True)
        (OUT / 'summary.json').write_text(json.dumps(summarize(records), indent=2) + '\n')
    print(json.dumps(summarize(records), indent=2), flush=True)

if __name__ == '__main__':
    main()
