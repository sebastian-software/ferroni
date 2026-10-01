#!/usr/bin/env python3
"""Compare Ferroni with other regex engines on one host.

Built for the manually dispatched Blacksmith workflow
(.github/workflows/blacksmith-comparison.yml) and runnable on any macOS or
Linux machine with the pinned Oniguruma and Onigmo sources. Case sets:

  trivial    patterns the `regex` crate also runs
  oniguruma  syntax only Oniguruma runs: lookbehind, backreferences, atomic
             groups, subexpression calls, absent operator, conditionals
  textmate   the TypeScript, CSS and Rust grammar scanners of battle_bench and
             replays of real Shiki scanner calls for C++, Java and SCSS
             (benches/*_scanner)

Engines: Ferroni, C Oniguruma (the vscode-oniguruma scanner for the replays),
Ruby's Onigmo (`onigmo` feature), PCRE2 with and without JIT, fancy-regex in
its Oniguruma mode, the `regex` crate where the syntax allows, and Shiki's
JavaScript engine for the replays (benches/shiki_js, Node). An engine is timed
only where it reproduced Oniguruma's (or the captured Shiki) results; the
summary lists every case an engine could not run, with the reason. The
binaries are ordinary release builds, as users run them.

  compare-engines.py run OUT [--cases trivial oniguruma ...]
      writes OUT/measurements.json, OUT/engine-notes.json, OUT/summary.md
  compare-engines.py report SUMMARY.md DIR...
      merges the measurements of several runs, one table set per host
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

# Ferroni benchmark IDs; the other engines' IDs are derived from them.
CASES = {
    'trivial': [
        'single_pattern/rust/literal_exact',
        'single_pattern/rust/quantifier_greedy',
        'single_pattern/rust/alternation_2_branch',
        'single_pattern/rust/alternation_10_branch',
        'single_pattern/rust/case_insensitive_phrase',
        'single_pattern/rust/named_capture_date',
        'single_pattern/rust/unicode_greek',
        'text_scanning/rust/literal_50k',
        'text_scanning/rust/no_match_50k',
        'text_scanning/rust/field_extract_50k',
        'text_scanning/rust/timestamp_50k',
        'general_regex/rust/email_validation',
        'general_regex/rust/uuid_validation',
        'general_regex/rust/number_validation',
        'general_regex/rust/access_log_captures',
        'general_regex/rust/url_extraction',
        'general_regex/rust/unicode_words',
        'general_regex/rust/email_redaction',
        'compilation/rust/literal',
        'compilation/rust/named_capture',
    ],
    'oniguruma': [
        'single_pattern/rust/lookaround_combined',
        'single_pattern/rust/backref_simple',
        'oniguruma_features/rust/atomic_possessive_strings',
        'oniguruma_features/rust/subexp_call_balanced',
        'oniguruma_features/rust/absent_comments',
        'oniguruma_features/rust/conditional_brackets',
        'oniguruma_features/rust/backref_ignorecase',
        'oniguruma_features/rust/lookbehind_alternation',
        'compilation/rust/lookbehind',
        'text_scanning/regset_position_lead_rust',
    ],
    'textmate': [
        'scanner_highlighting/ts_279_compile_rust',
        'scanner_highlighting/css_117_compile_rust',
        'scanner_highlighting/rust_81_compile_rust',
        'scanner_highlighting/ts_279_tokenize_rust',
        'scanner_highlighting/css_117_tokenize_rust',
        'scanner_highlighting/rust_81_tokenize_rust',
        'scanner_documents/ts_279_document_28_lines_rust',
        'scanner_documents/css_117_document_19_lines_rust',
        'scanner_documents/rust_81_document_31_lines_rust',
        # Captured Shiki scanner calls: the whole document, then the scanner
        # group that dominates it (see the README next to each trace).
        'cpp_scanner/document',
        'cpp_scanner/group_78',
        'java_scanner/document',
        'java_scanner/group_15',
        'scss_scanner/document',
        'scss_scanner/group_8',
    ],
}
# Criterion group prefix -> bench target; everything else is battle_bench.
BENCHES = {'cpp_scanner': 'cpp_scanner_bench', 'java_scanner': 'java_scanner_bench',
           'scss_scanner': 'scss_scanner_bench'}
ENGINES = ('rust', 'c', 'onigmo', 'pcre2_jit', 'pcre2', 'fancy_regex', 'regex', 'shiki_js')
LABELS = {'rust': 'Ferroni', 'c': 'C', 'onigmo': 'Onigmo', 'pcre2_jit': 'PCRE2 JIT', 'pcre2': 'PCRE2',
          'fancy_regex': 'fancy-regex', 'regex': 'regex', 'shiki_js': 'Shiki JS'}
SHIKI_JS = ROOT / 'benches' / 'shiki_js'
RUNNER_PROFILES = {
    'macos-arm64': ('Darwin', 'arm64'),
    'linux-x86-64': ('Linux', 'x86_64'),
}


def output(command, **kwargs):
    return subprocess.check_output(command, cwd=ROOT, text=True, **kwargs).strip()


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def bench_of(case):
    return BENCHES.get(case.split('/', 1)[0], 'battle_bench')


def variant(case, engine):
    if '/rust/' in case:
        return case.replace('/rust/', f'/{engine}/')
    if case.endswith('_rust'):
        return case[:-len('rust')] + engine
    # Scanner replays name Ferroni plainly and suffix the other engines.
    return case if engine == 'rust' else f'{case}_{engine}'


def slug(case):
    return case.replace('/rust/', '__').replace('/', '__').removesuffix('_rust')


def describe_host(runner_profile):
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin':
        cpu = output(['sysctl', '-n', 'machdep.cpu.brand_string'])
        memory = int(output(['sysctl', '-n', 'hw.memsize']))
        os_version = 'macOS ' + output(['sw_vers', '-productVersion'])
        features = ''
        if output(['sysctl', '-in', 'sysctl.proc_translated']) == '1':
            raise SystemExit('Rosetta translation would distort every timing.')
    elif system == 'Linux':
        cpuinfo = Path('/proc/cpuinfo').read_text()
        cpu = next((line.split(':', 1)[1].strip() for line in cpuinfo.splitlines()
                    if line.startswith('model name')), 'unknown')
        memory = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines()
                      if line.startswith('MemTotal:'))
        os_version = platform.freedesktop_os_release().get('PRETTY_NAME', 'Linux')
        features = ' '.join(sorted(set(re.findall(r'\b(sse4_2|avx2|avx512f|avx512bw|bmi2)\b', cpuinfo))))
    else:
        raise SystemExit('The comparison needs macOS or Linux.')
    if runner_profile and RUNNER_PROFILES[runner_profile] != (system, machine):
        raise SystemExit(f'Runner profile {runner_profile} expects {RUNNER_PROFILES[runner_profile]}, '
                         f'found {(system, machine)}.')
    return {
        'runner_profile': runner_profile, 'runner_label': os.environ.get('RUNNER_LABEL'),
        'system': system, 'machine': machine, 'cpu': cpu, 'cpu_features': features,
        'cpus': os.cpu_count(), 'memory_bytes': memory, 'os': os_version, 'kernel': platform.release(),
        'image': {key: os.environ.get(key) for key in ('ImageOS', 'ImageVersion')},
        'run_url': os.environ.get('COMPARISON_RUN_URL'),
    }


def build(target_dir, benches, onigmo):
    # The bundled PCRE2 of pcre2-sys, not whatever the host has installed.
    env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir), PCRE2_SYS_STATIC='1')
    command = ['cargo', 'bench', '--locked', '--features', 'onigmo' if onigmo else 'ffi', '--no-run',
               '--message-format=json']
    for bench in benches:
        command += ['--bench', bench]
    result = subprocess.run(command, cwd=ROOT, env=env, check=True, text=True, stdout=subprocess.PIPE)
    binaries = {}
    for line in result.stdout.splitlines():
        message = json.loads(line)
        if (message.get('reason') == 'compiler-artifact' and message['target']['name'] in benches
                and message.get('executable')):
            binaries[message['target']['name']] = Path(message['executable'])
    if set(binaries) != set(benches):
        raise SystemExit(f'cargo did not report executables for {set(benches) - set(binaries)}')
    receipt = {
        'git_commit': output(['git', 'rev-parse', 'HEAD']),
        'git_status': output(['git', 'status', '--porcelain']),
        'oniguruma_dir': os.environ.get('FERRONI_ONIGURUMA_DIR', '.cache/upstream/oniguruma-orig'),
        'onigmo_dir': os.environ.get('FERRONI_ONIGMO_DIR', '.cache/upstream/onigmo-ruby') if onigmo else None,
        'battle_inputs_sha256': sha256(ROOT / 'benches/battle_inputs.toml'),
        'lockfile_sha256': sha256(ROOT / 'Cargo.lock'),
        'binaries': {bench: {'path': str(path), 'sha256': sha256(path)} for bench, path in binaries.items()},
        'rustc': output(['rustc', '-vV']), 'cc': output(['cc', '--version']).splitlines()[0],
        'build_command': command, 'build_environment': {'PCRE2_SYS_STATIC': '1'},
    }
    return binaries, receipt


def run_bench(binary, args, out, log_name):
    env = dict(os.environ, CRITERION_HOME=str(out / 'criterion'))
    result = subprocess.run([str(binary), *args], cwd=out, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (out / 'logs').mkdir(exist_ok=True)
    (out / 'logs' / log_name).write_text(result.stdout)
    if result.returncode != 0:
        raise SystemExit(f'{" ".join(args)} failed; see logs/{log_name}:\n{result.stdout[-3000:]}')
    return result.stdout


def engine_notes(log):
    """UNSUPPORTED and EQUIVALENT lines the benches print during setup."""
    notes = {'unsupported': {}, 'equivalent': {}}
    for line in log.splitlines():
        kind, _, payload = line.partition(' ')
        if kind in ('UNSUPPORTED', 'EQUIVALENT'):
            note = json.loads(payload)
            notes[kind.lower()][note['id']] = note
    return notes


def shiki_js(case, mode, seconds=None):
    group, selection = case.split('/', 1)
    command = ['node', str(SHIKI_JS / 'replay.mjs'), group.removesuffix('_scanner'), selection, mode]
    if seconds:
        command.append(str(seconds))
    return json.loads(subprocess.check_output(command, cwd=SHIKI_JS, text=True).strip().splitlines()[-1])


def measure(binaries, out, case, engines, seconds, warm_up):
    rows = {}
    if 'shiki_js' in engines:
        timing = shiki_js(case, 'measure', seconds)
        rows['shiki_js'] = {'id': timing['id'], 'mean_ns': timing['mean_ns'], 'median_ns': timing['median_ns'],
                            'samples': timing['samples'], 'harness': 'node, 1 ms batches'}
        engines = [engine for engine in engines if engine != 'shiki_js']
    ids = [variant(case, engine) for engine in engines]
    pattern = '^(' + '|'.join(re.escape(i) for i in ids) + ')$'
    # 30 samples, as configure_battle_group sets for battle_bench; the replay
    # benches keep Criterion's default of 100, which would need about a minute
    # per document replay.
    run_bench(binaries[bench_of(case)],
              [pattern, '--bench', '--noplot', '--sample-size', '30', '--warm-up-time', str(warm_up),
               '--measurement-time', str(seconds)], out, slug(case) + '.log')
    for engine, bench_id in zip(engines, ids):
        estimates = json.loads((out / 'criterion' / bench_id / 'new' / 'estimates.json').read_text())
        rows[engine] = {'id': bench_id,
                        'mean_ns': estimates['mean']['point_estimate'],
                        'median_ns': estimates['median']['point_estimate'],
                        'mean_ci_ns': [estimates['mean']['confidence_interval']['lower_bound'],
                                       estimates['mean']['confidence_interval']['upper_bound']]}
    return rows


def ns(value):
    for unit, scale in (('s', 1e9), ('ms', 1e6), ('µs', 1e3)):
        if value >= scale:
            return f'{value / scale:.3g} {unit}'
    return f'{value:.3g} ns'


def short_reason(reason, width=180):
    reason = reason.replace('|', '\\|').replace('\n', ' ')
    return reason if len(reason) <= width else reason[:width - 1] + '…'


def render_summary(host, commit, results, notes):
    lines = [f'### Engine comparison: {host["runner_profile"] or host["system"]} ({host["cpu"]})', '',
             f'Source `{commit[:12]}`, {host["os"]}, {host["cpus"]} CPUs'
             + (f', {host["cpu_features"]}' if host['cpu_features'] else ''), '',
             'Ferroni is the Criterion mean. Every other column is that engine\'s time divided by '
             'Ferroni\'s: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case '
             '(reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.', '']
    engines = [engine for engine in ENGINES if engine != 'rust']
    for case_set in CASES:
        cases = results.get(case_set)
        if not cases:
            continue
        lines += [f'#### {case_set}', '',
                  '| case | Ferroni | ' + ' | '.join(LABELS[e] for e in engines) + ' |',
                  '| --- | ---: |' + ' ---: |' * len(engines)]
        for case, result in cases.items():
            timing = result.get('timing', {})
            rust = timing.get('rust', {}).get('mean_ns')
            cells = []
            for engine in engines:
                if engine in timing and rust:
                    cells.append(f'{timing[engine]["mean_ns"] / rust:.2f}')
                elif variant(case, engine) in notes['unsupported']:
                    cells.append('n/a')
                else:
                    cells.append('–')
            lines.append(f'| `{slug(case)}` | {ns(rust) if rust else "–"} | ' + ' | '.join(cells) + ' |')
        lines.append('')
    selected = {variant(case, engine) for cases in results.values() for case in cases for engine in ENGINES}
    unsupported = [note for bench_id, note in sorted(notes['unsupported'].items()) if bench_id in selected]
    if unsupported:
        lines += ['#### Cases an engine cannot run', '', '| benchmark | reason |', '| --- | --- |']
        lines += [f'| `{note["id"]}` | {short_reason(note["reason"])} |' for note in unsupported]
        lines.append('')
    equivalent = [note for bench_id, note in sorted(notes['equivalent'].items()) if bench_id in selected]
    if equivalent:
        lines += ['Accepted with empty capture groups reported as unset (or the reverse), which '
                  'vscode-textmate skips either way: '
                  + ', '.join(f'`{note["id"]}` ({note["empty_capture_differences"]} calls)' for note in equivalent), '']
    return '\n'.join(lines)


def run(args, parser):
    sets = list(CASES) if 'all' in args.cases else args.cases
    onigmo_dir = Path(os.environ.get('FERRONI_ONIGMO_DIR', ROOT / '.cache/upstream/onigmo-ruby'))
    onigmo = not args.no_onigmo
    if onigmo and not (onigmo_dir / 'regexec.c').is_file():
        parser.error('Onigmo sources are missing: run scripts/prepare-onigmo-sources.sh or pass --no-onigmo')
    use_shiki_js = not args.no_shiki_js
    if use_shiki_js and not (SHIKI_JS / 'node_modules').is_dir():
        parser.error('Run `pnpm install --frozen-lockfile --dir benches/shiki_js` or pass --no-shiki-js')

    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    host = describe_host(args.runner_profile)
    print(json.dumps(host, indent=2), flush=True)

    selected = {name: [case for case in CASES[name] if not args.only or args.only.search(case)] for name in sets}
    selected = {name: cases for name, cases in selected.items() if cases}
    if not selected:
        parser.error('no case matches --only')
    benches = sorted({bench_of(case) for cases in selected.values() for case in cases})
    binaries, receipt = build(args.target_dir.resolve(), benches, onigmo)
    if use_shiki_js:
        receipt['node'] = output(['node', '--version'])
        receipt['shiki_js_lock_sha256'] = sha256(SHIKI_JS / 'pnpm-lock.yaml')

    available = set()
    for binary in binaries.values():
        available |= {line.removesuffix(': benchmark') for line in
                      output([str(binary), '--list'], stderr=subprocess.DEVNULL).splitlines()
                      if line.endswith(': benchmark')}
    missing = [case for cases in selected.values() for case in cases if case not in available]
    if missing:
        raise SystemExit(f'The benches no longer have: {", ".join(missing)}')
    # One validation pass of every selected case before any timing. Setup
    # reports which engines could not reproduce a case.
    notes = {'unsupported': {}, 'equivalent': {}}
    for bench, binary in binaries.items():
        ids = [variant(case, engine) for cases in selected.values() for case in cases
               if bench_of(case) == bench for engine in ENGINES if variant(case, engine) in available]
        log = run_bench(binary, ['^(' + '|'.join(map(re.escape, ids)) + ')$', '--test'], out, f'validate-{bench}.log')
        for kind, found in engine_notes(log).items():
            notes[kind].update(found)
    if use_shiki_js:
        for case in [case for cases in selected.values() for case in cases if bench_of(case) != 'battle_bench']:
            check = shiki_js(case, 'validate')
            if check['status'] == 'ok':
                available.add(check['id'])
                if check['empty_capture_differences']:
                    notes['equivalent'][check['id']] = check
            else:
                notes['unsupported'][check['id']] = check
    (out / 'engine-notes.json').write_text(json.dumps(notes, indent=2) + '\n')

    results = {}
    settings = {'measure_seconds': args.measure_seconds, 'warm_up_seconds': args.warm_up_seconds,
                'sample_size': 30}
    for case_set, cases in selected.items():
        results[case_set] = {}
        for case in cases:
            engines = [engine for engine in ENGINES if variant(case, engine) in available]
            results[case_set][case] = {
                'engines': engines,
                'timing': measure(binaries, out, case, engines, args.measure_seconds, args.warm_up_seconds),
            }
            print(f'{case}: done', flush=True)
            (out / 'measurements.json').write_text(json.dumps(
                {'host': host, 'build': receipt, 'settings': settings, 'results': results, 'notes': notes},
                indent=2) + '\n')

    summary = render_summary(host, receipt['git_commit'], results, notes)
    (out / 'summary.md').write_text(summary + '\n')
    print(summary)


def report(args):
    """One summary per host from the measurements of several runs."""
    hosts = {}
    for directory in args.directories:
        for path in sorted(Path(directory).rglob('measurements.json')):
            data = json.loads(path.read_text())
            key = data['host']['runner_profile'] or f'{data["host"]["system"]} {data["host"]["cpu"]}'
            entry = hosts.setdefault(key, {'host': data['host'], 'commits': set(), 'results': {},
                                           'notes': {'unsupported': {}, 'equivalent': {}}})
            entry['commits'].add(data['build']['git_commit'])
            entry['results'].update(data['results'])
            for kind in entry['notes']:
                entry['notes'][kind].update(data['notes'][kind])
    if not hosts:
        raise SystemExit('No measurements.json found.')
    sections = []
    for entry in hosts.values():
        if len(entry['commits']) != 1:
            raise SystemExit(f'Runs for one host measured different commits: {sorted(entry["commits"])}')
        sections.append(render_summary(entry['host'], entry['commits'].pop(), entry['results'], entry['notes']))
    args.summary.write_text('\n'.join(sections) + '\n')
    print(args.summary.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest='command', required=True)
    run_parser = commands.add_parser('run', help='validate and time the selected cases on this host')
    run_parser.add_argument('output', type=Path)
    run_parser.add_argument('--cases', nargs='+', choices=(*CASES, 'all'), default=['all'])
    run_parser.add_argument('--measure-seconds', type=float, default=3)
    run_parser.add_argument('--warm-up-seconds', type=float, default=0.5)
    run_parser.add_argument('--runner-profile', choices=RUNNER_PROFILES)
    run_parser.add_argument('--only', type=re.compile, help='keep cases whose ID matches this regex')
    run_parser.add_argument('--no-onigmo', action='store_true', help='build without the onigmo feature')
    run_parser.add_argument('--no-shiki-js', action='store_true', help='skip the Node replays of Shiki\'s JS engine')
    run_parser.add_argument('--target-dir', type=Path, default=ROOT / 'target' / 'compare-engines')
    report_parser = commands.add_parser('report', help='merge the measurements of several runs')
    report_parser.add_argument('summary', type=Path)
    report_parser.add_argument('directories', nargs='+', type=Path)
    args = parser.parse_args()
    if args.command == 'run':
        if not 0.5 <= args.measure_seconds <= 30 or not 0.1 <= args.warm_up_seconds <= 5:
            run_parser.error('--measure-seconds must be 0.5-30 and --warm-up-seconds 0.1-5')
        run(args, run_parser)
    else:
        report(args)


if __name__ == '__main__':
    sys.exit(main())
