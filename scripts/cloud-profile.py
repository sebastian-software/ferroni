#!/usr/bin/env python3
"""Time and CPU-profile selected Ferroni benchmark cases on one host.

Built for the manually dispatched Blacksmith workflow
(.github/workflows/blacksmith-profile.yml), and runnable on any macOS or
Linux machine with samply and the pinned Oniguruma sources. Two case sets:

  trivial    patterns the `regex` crate also runs
  oniguruma  syntax only Oniguruma runs (lookbehind, backreferences, atomic
             groups, subexpression calls, absent operator, conditionals), the
             TypeScript, CSS and Rust grammar scanners of battle_bench, and
             replays of real Shiki scanner calls for C++, Java and SCSS
             (benches/*_scanner)

Engines: Ferroni, C Oniguruma (the vscode-oniguruma scanner for the replays),
Ruby's Onigmo (with the `onigmo` feature), PCRE2 with and without JIT,
fancy-regex in its Oniguruma mode, the `regex` crate where the syntax allows,
and Shiki's JavaScript engine for the replays (benches/shiki_js, Node). An
engine is timed only where it reproduced Oniguruma's (or the captured Shiki)
results; the summary lists every case an engine could not run, with the
reason. Every case is timed per engine, then sampled with samply for Ferroni
and C. Only samples inside Criterion's measurement routine are
attributed, so the per-process setup of every group (grammar compilation,
validation against C and the captured Shiki results) does not leak into the
hot-function tables.

Output directory layout:
  host.json, build.json       host, toolchain and binary receipt
  measurements.json           Criterion estimates per case and engine
  profiles/<case>.<engine>.json.gz (+ .syms.json)  open with `samply load`
  profiles/<case>.<engine>.txt                     self and inclusive tables
  summary.md                  the GitHub step summary
"""
from __future__ import annotations

import argparse
import bisect
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]

# Ferroni benchmark IDs; the C and regex counterparts are derived from them.
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
# Frames of Criterion's timed loop. Samples without one are process setup.
MEASURED_FRAME = re.compile(r'criterion::routine::')
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
    # Scanner replays name Ferroni plainly and suffix the C scanner.
    return case if engine == 'rust' else f'{case}_{engine}'


def describe_host(runner_profile):
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin':
        cpu = output(['sysctl', '-n', 'machdep.cpu.brand_string'])
        memory = int(output(['sysctl', '-n', 'hw.memsize']))
        os_version = 'macOS ' + output(['sw_vers', '-productVersion'])
        features = ''
        if output(['sysctl', '-in', 'sysctl.proc_translated']) == '1':
            raise SystemExit('Rosetta translation would distort every profile.')
    elif system == 'Linux':
        cpuinfo = Path('/proc/cpuinfo').read_text()
        cpu = next((line.split(':', 1)[1].strip() for line in cpuinfo.splitlines()
                    if line.startswith('model name')), 'unknown')
        memory = next(int(line.split()[1]) * 1024 for line in Path('/proc/meminfo').read_text().splitlines()
                      if line.startswith('MemTotal:'))
        os_version = platform.freedesktop_os_release().get('PRETTY_NAME', 'Linux')
        flags = set(re.findall(r'\b(sse4_2|avx2|avx512f|avx512bw|bmi2)\b', cpuinfo))
        features = ' '.join(sorted(flags))
    else:
        raise SystemExit('Profiling needs macOS or Linux.')
    if runner_profile and RUNNER_PROFILES[runner_profile] != (system, machine):
        raise SystemExit(f'Runner profile {runner_profile} expects {RUNNER_PROFILES[runner_profile]}, '
                         f'found {(system, machine)}.')
    return {
        'runner_profile': runner_profile, 'runner_label': os.environ.get('RUNNER_LABEL'),
        'system': system, 'machine': machine, 'cpu': cpu, 'cpu_features': features,
        'cpus': os.cpu_count(), 'memory_bytes': memory, 'os': os_version, 'kernel': platform.release(),
        'image': {key: os.environ.get(key) for key in ('ImageOS', 'ImageVersion')},
        'run_url': os.environ.get('PROFILE_RUN_URL'),
    }


def build(target_dir, benches, onigmo):
    env = dict(os.environ,
               CARGO_TARGET_DIR=str(target_dir),
               # The bundled PCRE2 of pcre2-sys, not whatever the host has.
               PCRE2_SYS_STATIC='1',
               CARGO_PROFILE_BENCH_DEBUG='line-tables-only',
               CARGO_PROFILE_BENCH_STRIP='none',
               RUSTFLAGS=(os.environ.get('RUSTFLAGS', '') + ' -C force-frame-pointers=yes').strip(),
               CFLAGS=(os.environ.get('CFLAGS', '') + ' -fno-omit-frame-pointer').strip())
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
        'samply': output(['samply', '--version']),
        'build_command': command,
        'build_environment': {key: env[key] for key in env
                              if key.startswith('CARGO_PROFILE_') or key in ('RUSTFLAGS', 'CFLAGS', 'PCRE2_SYS_STATIC')},
    }
    return binaries, receipt


def run_bench(binary, args, out, log_name):
    env = dict(os.environ, CRITERION_HOME=str(out / 'criterion'))
    started = time.monotonic()
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


def measure(binaries, out, case, engines, seconds):
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
    run_bench(binaries[bench_of(case)], [pattern, '--bench', '--noplot', '--sample-size', '30', '--warm-up-time', '1',
                       '--measurement-time', str(seconds)], out, slug(case) + '.measure.log')
    for engine, bench_id in zip(engines, ids):
        estimates = json.loads((out / 'criterion' / bench_id / 'new' / 'estimates.json').read_text())
        rows[engine] = {'id': bench_id,
                        'mean_ns': estimates['mean']['point_estimate'],
                        'median_ns': estimates['median']['point_estimate'],
                        'mean_ci_ns': [estimates['mean']['confidence_interval']['lower_bound'],
                                       estimates['mean']['confidence_interval']['upper_bound']]}
    return rows


class Symbols:
    """Address lookup in the .syms.json sidecar of `samply --unstable-presymbolicate`."""

    def __init__(self, path):
        data = json.loads(Path(path).read_text())
        strings = data['string_table']
        self.libs = {}
        for lib in data['data']:
            table = sorted(lib['symbol_table'], key=lambda entry: entry['rva'])
            self.libs[lib['code_id'].upper()] = (
                [entry['rva'] for entry in table],
                [(entry['rva'] + entry['size'], strings[entry['symbol']]) for entry in table])

    def name(self, lib, address):
        tables = self.libs.get((lib.get('codeId') or '').upper())
        if tables:
            starts, ends = tables
            index = bisect.bisect_right(starts, address) - 1
            if index >= 0 and address < ends[index][0]:
                return ends[index][1]
        return f'{lib.get("name", "?")}+{address:#x}'


def summarize_profile(profile_path, top):
    profile = json.loads(gzip.decompress(profile_path.read_bytes()))
    symbols = Symbols(str(profile_path)[:-len('.json.gz')] + '.json.syms.json')
    thread = next(t for t in profile['threads'] if t.get('isMainThread'))
    frames, funcs, resources = thread['frameTable'], thread['funcTable'], thread['resourceTable']
    libs = profile['libs']

    names = []
    for index in range(frames['length']):
        resource = funcs['resource'][frames['func'][index]]
        lib_index = resources['lib'][resource] if resource is not None and resource >= 0 else None
        address = frames['address'][index]
        if lib_index is None or address is None or address < 0:
            names.append(thread['stringArray'][funcs['name'][frames['func'][index]]])
        else:
            names.append(symbols.name(libs[lib_index], address))

    stacks = thread['stackTable']
    samples = thread['samples']
    weights = samples.get('weight') or [1] * samples['length']
    self_time, inclusive, measured, total = {}, {}, 0, 0
    cache = {}
    for stack, weight in zip(samples['stack'], weights):
        if stack is None:
            continue
        total += weight
        if stack not in cache:
            chain, cursor = [], stack
            while cursor is not None:
                chain.append(names[stacks['frame'][cursor]])
                cursor = stacks['prefix'][cursor]
            cache[stack] = chain
        chain = cache[stack]
        if not any(MEASURED_FRAME.search(name) for name in chain):
            continue
        measured += weight
        self_time[chain[0]] = self_time.get(chain[0], 0) + weight
        for name in set(chain):
            inclusive[name] = inclusive.get(name, 0) + weight

    def table(counts):
        rows = sorted(counts.items(), key=lambda item: -item[1])[:top]
        return [{'percent': round(100 * count / measured, 2), 'samples': count, 'function': name}
                for name, count in rows] if measured else []

    return {'samples': total, 'measured_samples': measured,
            'self': table(self_time), 'inclusive': table(inclusive)}


def profile_case(binaries, out, case, engine, seconds, top):
    bench_id = variant(case, engine)
    base = out / 'profiles' / f'{slug(case)}.{engine}'
    base.parent.mkdir(exist_ok=True)
    record = ['samply', 'record', '--save-only', '--unstable-presymbolicate', '--main-thread-only',
              '--rate', '1000', '-o', f'{base}.json.gz', str(binaries[bench_of(case)]),
              '^' + re.escape(bench_id) + '$', '--bench', '--profile-time', str(seconds)]
    env = dict(os.environ, CRITERION_HOME=str(out / 'criterion-profile'))
    result = subprocess.run(record, cwd=out, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (out / 'logs').mkdir(exist_ok=True)
    (out / 'logs' / f'{base.name}.profile.log').write_text(result.stdout)
    if result.returncode != 0:
        raise SystemExit(f'samply failed for {bench_id}:\n{result.stdout[-3000:]}')
    summary = summarize_profile(Path(f'{base}.json.gz'), top)
    lines = [f'{bench_id}: {summary["measured_samples"]} of {summary["samples"]} samples in the measured loop', '']
    for kind in ('self', 'inclusive'):
        lines.append(f'{kind} time')
        lines += [f'{row["percent"]:6.2f}%  {row["function"]}' for row in summary[kind]]
        lines.append('')
    Path(f'{base}.txt').write_text('\n'.join(lines))
    return {'id': bench_id, 'profile': f'profiles/{base.name}.json.gz', **summary}


def slug(case):
    return case.replace('/rust/', '__').replace('/', '__').removesuffix('_rust')


def short(name, width=90):
    name = re.sub(r'<[^<>]*(?:<[^<>]*>[^<>]*)*>', '<…>', name)
    return name if len(name) <= width else name[:width - 1] + '…'


def ns(value):
    for unit, scale in (('s', 1e9), ('ms', 1e6), ('µs', 1e3)):
        if value >= scale:
            return f'{value / scale:.3g} {unit}'
    return f'{value:.3g} ns'


def render_summary(host, receipt, results, notes):
    lines = [f'### Ferroni profile: {host["runner_profile"] or host["system"]} ({host["cpu"]})', '',
             f'Source `{receipt["git_commit"][:12]}`, {host["os"]}, {host["cpus"]} CPUs'
             + (f', {host["cpu_features"]}' if host['cpu_features'] else ''), '',
             'Ferroni is the Criterion mean. Every other column is that engine\'s time divided by '
             'Ferroni\'s: above 1.00, Ferroni is faster. `n/a`: the engine cannot run the case '
             '(reasons below); `–`: no such variant. Shiki JS is timed in Node over 1 ms batches.', '']
    engines = [engine for engine in ENGINES if engine != 'rust']
    for case_set, cases in results.items():
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
    profiled = [(case, result['profile']['rust']) for cases in results.values()
                for case, result in cases.items() if result.get('profile', {}).get('rust')]
    if profiled:
        lines += ['#### Ferroni hot spots (self time in the measured loop)', '',
                  '| case | functions |', '| --- | --- |']
        lines += [f'| `{slug(case)}` | ' + '<br>'.join(f'{row["percent"]:.0f}% `{short(row["function"], 70)}`'
                                                     for row in profile['self'][:3]) + ' |'
                  for case, profile in profiled]
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


def short_reason(reason, width=180):
    reason = reason.replace('|', '\\|').replace('\n', ' ')
    return reason if len(reason) <= width else reason[:width - 1] + '…'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cases', choices=('trivial', 'oniguruma', 'all'), default='all')
    parser.add_argument('--measure-seconds', type=int, default=5)
    parser.add_argument('--profile-seconds', type=int, default=15)
    parser.add_argument('--profile-engines', default='rust,c',
                        help='comma-separated engines to sample; any but shiki_js')
    parser.add_argument('--no-onigmo', action='store_true', help='build without the onigmo feature')
    parser.add_argument('--no-shiki-js', action='store_true', help='skip the Node replays of Shiki\'s JS engine')
    parser.add_argument('--skip-measure', action='store_true')
    parser.add_argument('--skip-profile', action='store_true')
    parser.add_argument('--runner-profile', choices=RUNNER_PROFILES)
    parser.add_argument('--only', type=re.compile, help='keep cases whose ID matches this regex')
    parser.add_argument('--top', type=int, default=25)
    parser.add_argument('--target-dir', type=Path, default=ROOT / 'target' / 'cloud-profile')
    args = parser.parse_args()
    if not 1 <= args.measure_seconds <= 30 or not 2 <= args.profile_seconds <= 120:
        parser.error('--measure-seconds must be 1-30 and --profile-seconds 2-120')
    profile_engines = [engine for engine in args.profile_engines.split(',') if engine]
    if not set(profile_engines) <= set(ENGINES) - {'shiki_js'}:
        parser.error(f'--profile-engines takes {", ".join(ENGINES[:-1])}')
    onigmo_dir = Path(os.environ.get('FERRONI_ONIGMO_DIR', ROOT / '.cache/upstream/onigmo-ruby'))
    onigmo = not args.no_onigmo
    if onigmo and not (onigmo_dir / 'regexec.c').is_file():
        parser.error('Onigmo sources are missing: run scripts/prepare-onigmo-sources.sh or pass --no-onigmo')
    use_shiki_js = not args.no_shiki_js
    if use_shiki_js and not (SHIKI_JS / 'node_modules').is_dir():
        parser.error('Run `pnpm install --frozen-lockfile` in benches/shiki_js or pass --no-shiki-js')

    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    host = describe_host(args.runner_profile)
    (out / 'host.json').write_text(json.dumps(host, indent=2) + '\n')
    print(json.dumps(host, indent=2), flush=True)

    selected = {name: [case for case in cases if not args.only or args.only.search(case)]
                for name, cases in CASES.items() if args.cases in (name, 'all')}
    selected = {name: cases for name, cases in selected.items() if cases}
    if not selected:
        parser.error('no case matches --only')
    benches = sorted({bench_of(case) for cases in selected.values() for case in cases})
    binaries, receipt = build(args.target_dir.resolve(), benches, onigmo)
    if use_shiki_js:
        receipt['node'] = output(['node', '--version'])
        receipt['shiki_js_lock_sha256'] = sha256(SHIKI_JS / 'pnpm-lock.yaml')
    (out / 'build.json').write_text(json.dumps(receipt, indent=2) + '\n')

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
        log = run_bench(binary, ['^(' + '|'.join(map(re.escape, ids)) + ')$', '--test'], out, f'smoke-{bench}.log')
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
    for case_set, cases in selected.items():
        results[case_set] = {}
        for case in cases:
            engines = [engine for engine in ENGINES if variant(case, engine) in available]
            result = results[case_set][case] = {'engines': engines}
            if not args.skip_measure:
                result['timing'] = measure(binaries, out, case, engines, args.measure_seconds)
            if not args.skip_profile:
                result['profile'] = {engine: profile_case(binaries, out, case, engine, args.profile_seconds, args.top)
                                     for engine in profile_engines if engine in engines}
            print(f'{case}: done', flush=True)
            (out / 'measurements.json').write_text(json.dumps(
                {'host': host, 'build': receipt, 'results': results, 'notes': notes}, indent=2) + '\n')

    summary = render_summary(host, receipt, results, notes)
    (out / 'summary.md').write_text(summary + '\n')
    print(summary)


if __name__ == '__main__':
    sys.exit(main())
