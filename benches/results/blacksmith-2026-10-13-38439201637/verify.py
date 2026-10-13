"""Verify the retained full-run receipts before publishing any figures."""
from pathlib import Path
import hashlib
import json
import math
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
dispatch = json.loads((ROOT / 'dispatch.json').read_text())
run = json.loads((ROOT / 'run.json').read_text())
expected = dispatch['expected_main_sha']
prior = {(data['profile'], data['group']): data
         for data in json.loads((ROOT / 'analysis/pre-css-receipts.json').read_text())}
assert run['headSha'] == expected and run['status'] == 'completed'
assert run['conclusion'] == 'success' and run['event'] == 'workflow_dispatch'
assert len(run['jobs']) == 9 and all(j['conclusion'] == 'success' for j in run['jobs'])

seen = set()
jobs = []
for path in sorted((ROOT / 'raw').rglob('measurements.json')):
    data = json.loads(path.read_text())
    build = data['build']
    assert build['git_commit'] == expected and not build['git_status'], path
    assert build['lockfile_sha256'] == dispatch['lockfile_sha256'], path
    assert build['battle_inputs_sha256'] == dispatch['inputs_sha256'], path
    assert data['host']['run_url'] == dispatch['run_url'], path
    assert data['settings'] == {'measure_seconds': 5.0, 'warm_up_seconds': 0.5, 'sample_size': 30}, path
    assert len(data['results']) == 1, path
    group, cases = next(iter(data['results'].items()))
    profile = data['host']['runner_profile']
    key = (profile, group)
    assert key not in seen, key
    seen.add(key)
    previous = prior[key]
    host_changes = {field: {'previous': previous['host'][field], 'current': data['host'][field]}
                    for field in ['cpu', 'cpu_features', 'memory_bytes', 'os']
                    if data['host'][field] != previous['host'][field]}
    for field in ['system', 'machine', 'cpus', 'runner_label']:
        assert data['host'][field] == previous['host'][field], (key, field)
    for field in ['rustc', 'cc', 'node']:
        assert build.get(field) == previous['build'].get(field), (key, field)
    assert set(data['notes']['unsupported']) == set(previous['notes']['unsupported']), key
    binary_changes = {name: receipt['sha256'] != previous['build']['binaries'][name]['sha256']
                      for name, receipt in build['binaries'].items()}
    assert all(binary_changes.values()), (key, 'unchanged executable')
    for case, row in cases.items():
        assert 'rust' in row['timing'], (profile, case)
        for engine, timing in row['timing'].items():
            assert math.isfinite(timing['mean_ns']) and timing['mean_ns'] > 0, (case, engine)
            assert timing['id'] not in data['notes']['unsupported'], (case, engine)
    jobs.append({'profile': profile, 'group': group, 'cases': len(cases),
                 'accepted_variants': sum(len(c['timing']) for c in cases.values()),
                 'host': data['host'], 'rustc': build['rustc'], 'cc': build['cc'],
                 'node': build.get('node'), 'source_commit': build['git_commit'],
                 'same_toolchain_as_pre_css': True, 'host_changes_from_pre_css': host_changes,
                 'different_executable_hashes_from_pre_css': binary_changes})

assert seen == {(p, g) for p in ['macos-arm64', 'linux-x86-64']
                for g in ['shared', 'oniguruma', 'micro', 'textmate']}
assert sum(j['cases'] for j in jobs) == 170
source_tree = subprocess.check_output(['git', 'rev-parse', expected + ':src'], cwd=REPO, text=True).strip()
assert source_tree == dispatch['source_tree']
fixture_hashes = {}
for p in ['benches/battle_inputs.toml', 'Cargo.lock', 'scripts/compare-engines.py',
          'benches/regex_tasks.rs', 'benches/general_regex.rs', 'benches/engines.rs',
          'benches/shiki_js/pnpm-lock.yaml',
          'benches/battle_bench.rs', 'benches/grammar_loader.rs',
          'benches/scanner_documents.rs', 'benches/scanner_css_workload.rs',
          *[f'benches/grammars/{lang}.json' for lang in ['typescript', 'css', 'rust']],
          *[f'benches/{lang}_scanner/trace.json' for lang in ['cpp', 'java', 'scss', 'c', 'php', 'json', 'astro']]]:
    content = subprocess.check_output(['git', 'show', expected + ':' + p], cwd=REPO)
    fixture_hashes[p] = hashlib.sha256(content).hexdigest()
(ROOT / 'analysis/integrity.json').write_text(json.dumps({
    'eight_complete_jobs': True, 'verified_source_commit': expected,
    'source_tree': source_tree, 'jobs': jobs, 'measured_fixture_sha256': fixture_hashes,
}, indent=2) + '\n')
print('Verified eight clean builds, 170 cases, exact main source, fixtures and successful jobs.')
