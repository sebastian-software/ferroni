#!/usr/bin/env python3
"""Time and CPU-profile selected Ferroni benchmark cases on one host.

Built for the manually dispatched Blacksmith workflow
(.github/workflows/blacksmith-profile.yml), and runnable on any macOS or
Linux machine with samply and the pinned Oniguruma sources. Two case sets:

  trivial    patterns the `regex` crate also runs: Ferroni, C Oniguruma, regex
  oniguruma  syntax only Oniguruma runs (lookbehind, backreferences, atomic
             groups, subexpression calls, absent operator, conditionals), the
             TypeScript, CSS and Rust grammar scanners of battle_bench, and
             replays of real Shiki scanner calls for C++, Java and SCSS
             (benches/*_scanner), each against the vscode-oniguruma C scanner

Every case is timed with Criterion for each engine, then sampled with samply
for Ferroni and C. Only samples inside Criterion's measurement routine are
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
ENGINES = ('rust', 'c', 'regex')
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


def build(target_dir, benches):
    env = dict(os.environ,
               CARGO_TARGET_DIR=str(target_dir),
               CARGO_PROFILE_BENCH_DEBUG='line-tables-only',
               CARGO_PROFILE_BENCH_STRIP='none',
               RUSTFLAGS=(os.environ.get('RUSTFLAGS', '') + ' -C force-frame-pointers=yes').strip(),
               CFLAGS=(os.environ.get('CFLAGS', '') + ' -fno-omit-frame-pointer').strip())
    command = ['cargo', 'bench', '--locked', '--features', 'ffi', '--no-run', '--message-format=json']
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
        'battle_inputs_sha256': sha256(ROOT / 'benches/battle_inputs.toml'),
        'lockfile_sha256': sha256(ROOT / 'Cargo.lock'),
        'binaries': {bench: {'path': str(path), 'sha256': sha256(path)} for bench, path in binaries.items()},
        'rustc': output(['rustc', '-vV']), 'cc': output(['cc', '--version']).splitlines()[0],
        'samply': output(['samply', '--version']),
        'build_command': command,
        'build_environment': {key: env[key] for key in env
                              if key.startswith('CARGO_PROFILE_') or key in ('RUSTFLAGS', 'CFLAGS')},
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
    return time.monotonic() - started


def measure(binaries, out, case, engines, seconds):
    ids = [variant(case, engine) for engine in engines]
    pattern = '^(' + '|'.join(re.escape(i) for i in ids) + ')$'
    # 30 samples, as configure_battle_group sets for battle_bench; the replay
    # benches keep Criterion's default of 100, which would need about a minute
    # per document replay.
    run_bench(binaries[bench_of(case)], [pattern, '--bench', '--noplot', '--sample-size', '30', '--warm-up-time', '1',
                       '--measurement-time', str(seconds)], out, slug(case) + '.measure.log')
    rows = {}
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


def render_summary(host, receipt, results):
    lines = [f'### Ferroni profile: {host["runner_profile"] or host["system"]} ({host["cpu"]})', '',
             f'Source `{receipt["git_commit"][:12]}`, {host["os"]}, {host["cpus"]} CPUs'
             + (f', {host["cpu_features"]}' if host['cpu_features'] else ''), '',
             'Times are Criterion means; ratios above 1.00 mean Ferroni is slower. '
             'Hot functions are self time of Ferroni samples inside the measured loop.', '']
    for case_set, cases in results.items():
        lines += [f'#### {case_set}', '',
                  '| case | Ferroni | C | regex | Ferroni/C | Ferroni/regex | Ferroni hot spots (self) |',
                  '| --- | ---: | ---: | ---: | ---: | ---: | --- |']
        for case, result in cases.items():
            timing = result.get('timing', {})
            rust = timing.get('rust', {}).get('mean_ns')
            cells = [ns(timing[e]['mean_ns']) if e in timing else '–' for e in ENGINES]
            ratios = [f'{rust / timing[e]["mean_ns"]:.2f}' if rust and e in timing else '–'
                      for e in ('c', 'regex')]
            profile = result.get('profile', {}).get('rust')
            hot = '<br>'.join(f'{row["percent"]:.0f}% `{short(row["function"], 60)}`'
                              for row in (profile or {}).get('self', [])[:3]) or '–'
            lines.append(f'| `{slug(case)}` | ' + ' | '.join(cells + ratios) + f' | {hot} |')
        lines.append('')
    return '\n'.join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cases', choices=('trivial', 'oniguruma', 'all'), default='all')
    parser.add_argument('--measure-seconds', type=int, default=5)
    parser.add_argument('--profile-seconds', type=int, default=15)
    parser.add_argument('--profile-engines', default='rust,c',
                        help='comma-separated engines to sample (rust, c, regex)')
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
    if not set(profile_engines) <= set(ENGINES):
        parser.error(f'--profile-engines takes {", ".join(ENGINES)}')

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
    binaries, receipt = build(args.target_dir.resolve(), benches)
    (out / 'build.json').write_text(json.dumps(receipt, indent=2) + '\n')

    available = set()
    for binary in binaries.values():
        available |= {line.removesuffix(': benchmark') for line in
                      output([str(binary), '--list'], stderr=subprocess.DEVNULL).splitlines()
                      if line.endswith(': benchmark')}
    missing = [case for cases in selected.values() for case in cases if case not in available]
    if missing:
        raise SystemExit(f'The benches no longer have: {", ".join(missing)}')
    # One validation pass of every selected case before any timing.
    for bench, binary in binaries.items():
        ids = [variant(case, engine) for cases in selected.values() for case in cases
               if bench_of(case) == bench for engine in ENGINES if variant(case, engine) in available]
        run_bench(binary, ['^(' + '|'.join(map(re.escape, ids)) + ')$', '--test'], out, f'smoke-{bench}.log')

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
                {'host': host, 'build': receipt, 'results': results}, indent=2) + '\n')

    summary = render_summary(host, receipt, results)
    (out / 'summary.md').write_text(summary + '\n')
    print(summary)


if __name__ == '__main__':
    sys.exit(main())
