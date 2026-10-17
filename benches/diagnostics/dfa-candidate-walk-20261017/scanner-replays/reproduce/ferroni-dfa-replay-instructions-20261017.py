import gzip
import json
import os
from pathlib import Path
import re
import statistics
import subprocess

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')

def main():
    assert len(json.loads((OUT / 'measurements.json').read_text())) == 128
    target = OUT / 'instructions'
    target.mkdir()
    plan = {
        'question': 'Does the optimization reduce retired instructions on the complete real scanner traces?',
        'purpose': 'Mechanism diagnostic after noisy wall-time controls; not a public highlighting speed claim.',
        'cases': ['cpp', 'tsx'], 'repetitions': 2, 'driver_seconds': [1, 5],
        'method': 'For each executable, subtract the short process retired instructions from the long process, then divide by the difference in completed replays. Initialization and validation are repeated in both processes.',
        'boundary': 'Includes fresh string construction and loop bookkeeping, which Criterion excludes. Uses ordinary release drivers with no instrumentation.',
        'limitations': 'The timer and timing-vector storage add a small per-replay cost. Different replay counts can change allocator and vector-capacity behavior. Hardware counters are diagnostic only.',
    }
    (target / 'plan.json').write_text(json.dumps(plan, indent=2) + '\n')
    rows = []
    for rep in range(2):
        for language in ['cpp', 'tsx']:
            variants = ['baseline', 'candidate'] if rep == 0 else ['candidate', 'baseline']
            for variant in variants:
                cells = {}
                durations = [1, 5] if rep == 0 else [5, 1]
                for duration in durations:
                    name = f'{language}-{variant}-{rep+1}-{duration}s'
                    binary = OUT / 'binaries' / variant / 'profile_cpp_scanner'
                    command = ['/usr/bin/time', '-l', str(binary), str(duration), 'all', str(ROOT / f'benches/{language}_scanner/trace.json')]
                    result = subprocess.run(command, cwd=ROOT, env=dict(os.environ, LC_ALL='C'), text=True,
                                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=60)
                    with gzip.open(target / (name + '.json.gz'), 'wt') as archive:
                        archive.write(result.stdout)
                    (target / (name + '.log')).write_text(result.stderr)
                    data = json.loads(result.stdout)
                    match = re.search(r'^\s*(\d+)\s+instructions retired\s*$', result.stderr, re.MULTILINE)
                    if match is None:
                        raise RuntimeError('Hardware instruction counts unavailable; retained output is not an instruction measurement')
                    cells[duration] = {'instructions': int(match.group(1)), 'replays': len(data['timings_ns']), 'command': command}
                assert cells[5]['replays'] > cells[1]['replays']
                per_replay = (cells[5]['instructions'] - cells[1]['instructions']) / (cells[5]['replays'] - cells[1]['replays'])
                row = {'case': language, 'variant': variant, 'repetition': rep+1, 'cells': cells, 'instructions_per_replay': per_replay}
                rows.append(row)
                (target / 'measurements.json').write_text(json.dumps(rows, indent=2) + '\n')
                print(f'{language} {variant} repetition {rep+1}: {per_replay:.0f} instructions/replay', flush=True)
    summary = []
    for language in ['cpp', 'tsx']:
        mine = [r for r in rows if r['case'] == language]
        variants = {variant: statistics.median(r['instructions_per_replay'] for r in mine if r['variant'] == variant) for variant in ['baseline', 'candidate']}
        summary.append({'case': language, **variants, 'change_pct': (variants['candidate'] / variants['baseline'] - 1) * 100})
    (target / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2), flush=True)

if __name__ == '__main__':
    main()
