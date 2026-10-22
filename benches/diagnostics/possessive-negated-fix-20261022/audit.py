#!/usr/bin/env python3
"""Audit completeness, counterbalancing, raw samples, and serial process timing."""
import argparse
from datetime import datetime
import json
from pathlib import Path


def audit(directory):
    context = json.loads((directory / 'context.json').read_text())
    rows = json.loads((directory / 'measurements.json').read_text())
    summary = json.loads((directory / 'summary.json').read_text())
    cases = {row['case'] for row in summary}
    if context['args']['cases'] == 'all':
        repo = Path(__file__).resolve().parents[3]
        requested = {case['id'] for folder in ['production_regex', 'tokenizer_regex']
                     for case in json.loads((repo / 'benches' / folder / 'catalog.json').read_text())['cases']
                     if case.get('inputs')}
    else:
        requested = set(context['args']['cases'].split(','))
    assert cases == requested
    repetitions = context['args']['repetitions']
    offsets = [int(value) for value in context['args']['alignment_offsets'].split(',')]
    assert context['environment_sizes'] == [context['environment_base_bytes'] + offsets[n % len(offsets)]
                                            for n in range(repetitions)]
    assert len(rows) == len(cases) * repetitions * 2
    assert len(context['order']) == len(rows)
    seen = set()
    paired_orders = {}
    previous_end = None
    sample_count = 0
    for expected, row in zip(context['order'], rows):
        key = (row['repetition'], row['case'], row['variant'])
        assert key not in seen
        seen.add(key)
        assert row['engine'] == 'rust' and row['variant'] in [0, 1]
        assert set(row['lanes']) == {'steady'}
        for field in ['repetition', 'case', 'engine', 'variant', 'alignment_offset']:
            assert expected[field] == row[field]
        assert row['alignment_offset'] == offsets[row['repetition'] % len(offsets)]
        start = datetime.fromisoformat(row['started_utc'])
        end = datetime.fromisoformat(row['finished_utc'])
        assert start <= end and (previous_end is None or previous_end <= start)
        previous_end = end
        paired_orders.setdefault((row['case'], row['repetition']), []).append(row['variant'])
        raw = directory / f"raw/{row['repetition']:02d}/{row['case']}/rust/{row['variant']}"
        assert (raw / 'process.log').is_file()
        estimate = row['lanes']['steady']
        samples = json.loads((raw / estimate['raw_directory'] / 'sample.json').read_text())
        assert len(samples['times']) == context['args']['samples']
        assert len(samples['iters']) == len(samples['times'])
        assert min(samples['times']) > 0 and min(samples['iters']) > 0
        assert estimate['ns'] > 0
        sample_count += len(samples['times'])
    for case in cases:
        for repetition in range(repetitions):
            assert sorted(paired_orders[case, repetition]) == [0, 1]
        if repetitions == 2 * len(offsets):
            for index in range(len(offsets)):
                assert paired_orders[case, index] == list(reversed(paired_orders[case, index + len(offsets)]))
    return {'directory': directory.name, 'cases': len(cases), 'pairs': len(rows) // 2,
            'processes': len(rows), 'criterion_samples': sample_count, 'complete': True,
            'started_utc': rows[0]['started_utc'], 'finished_utc': rows[-1]['finished_utc'],
            'load_average_1m_range': [min(row['load_average'][0] for row in rows),
                                      max(row['load_average'][0] for row in rows)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('directories', nargs='+')
    args = parser.parse_args()
    results = [audit(args.root / name) for name in args.directories]
    ordered = sorted(results, key=lambda row: row['started_utc'])
    for before, after in zip(ordered, ordered[1:]):
        assert before['finished_utc'] <= after['started_utc']
    args.output.write_text(json.dumps(results, indent=2) + '\n')
    print(sum(row['processes'] for row in results), 'timed processes;',
          sum(row['pairs'] for row in results), 'pairs;',
          sum(row['criterion_samples'] for row in results), 'samples; complete and serial')


if __name__ == '__main__':
    main()
