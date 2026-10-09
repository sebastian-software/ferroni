#!/usr/bin/env python3
"""Compare Ferroni with other regex engines on one host.

Built for the manually dispatched Blacksmith workflow
(.github/workflows/blacksmith-comparison.yml) and runnable on any macOS or
Linux machine with the pinned Oniguruma and Onigmo sources. Case sets:

  shared     everyday text processing the `regex` crate also runs: markup,
             logs, chat with emoji, Markdown, JSON, CSV, validation
  oniguruma  the same kind of work with Oniguruma syntax: lookaround,
             backreferences, possessive and atomic groups, subexpression
             calls, the absent operator, conditionals, grapheme clusters
  micro      one search on a short string, and compiling single patterns
  textmate   the TypeScript, CSS and Rust grammar scanners of battle_bench and
             replays of real Shiki scanner calls (benches/*_scanner)

Engines: Ferroni, C Oniguruma (the vscode-oniguruma scanner for the replays),
Ruby's Onigmo (`onigmo` feature), PCRE2 with and without JIT, fancy-regex in
its Oniguruma mode with and without seek mode (fancy_regex_seek), fancy-regex's
RegexSet over each scanner's patterns (fancy_regex_set, scanner replays only),
the `regex` crate where the syntax allows, and Shiki's JavaScript engine for
the replays (benches/shiki_js, Node). An engine is timed only where it
reproduced Oniguruma's (or the captured Shiki) results; the summary lists every
case an engine could not run, with the reason. The binaries are ordinary
release builds, as users run them.

  compare-engines.py run OUT [--cases shared oniguruma ...] [--engines ...]
      writes OUT/measurements.json, OUT/engine-notes.json, OUT/summary.md
      --engines times only the named engines next to Ferroni, for local runs
  compare-engines.py report SUMMARY.md DIR...
      merges the measurements of several runs, one table set per host
  compare-engines.py figures RUN_DIR... OUT.json
      condenses the published runs into the README and home page figures
  compare-engines.py tables RUN_DIR...
      prints the per-case tables of docs/app/routes/perf/engine-comparison.mdx

figures and tables take several published runs in order: a later run's case
sets replace the earlier run's, so a case set measured again after a fix is
published without editing numbers by hand.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

# Ferroni benchmark IDs; the other engines' IDs are derived from them.
REGEX_TASKS_SHARED = ['html_tags', 'html_attributes', 'html_comments', 'hex_colors', 'email_addresses', 'emoji',
                      'ascii_emoticons', 'hashtags_mentions', 'ipv4_addresses', 'iso_timestamps',
                      'log_keywords_ignorecase', 'markdown_links', 'semantic_versions', 'json_strings', 'csv_fields',
                      # At the limit: whole-grammar expressions, large alternations, backtracking, full
                      # Unicode case folding.
                      'rfc5322_emails', 'ipv6_addresses', 'rfc3986_urls', 'keyword_alternation_200',
                      'csv_last_column_backtracking', 'unicode_case_folding']
REGEX_TASKS_ONIGURUMA = ['html_element_pairs', 'html_attribute_values', 'prices_lookbehind', 'quoted_strings',
                         'camel_case_words', 'markdown_emphasis', 'emoji_graphemes', 'password_rules',
                         'json_objects_recursive', 'html_nested_divs_recursive', 'variable_lookbehind']
CASES = {
    # Everyday text processing on documents of a few dozen kilobytes.
    'shared': [
        *[f'regex_tasks/rust/{name}' for name in REGEX_TASKS_SHARED],
        'general_regex/rust/email_validation',
        'general_regex/rust/uuid_validation',
        'general_regex/rust/number_validation',
        'general_regex/rust/access_log_captures',
        'general_regex/rust/url_extraction',
        'general_regex/rust/unicode_words',
        'general_regex/rust/email_redaction',
        'text_scanning/rust/literal_50k',
        'text_scanning/rust/no_match_50k',
        'text_scanning/rust/field_extract_50k',
        'text_scanning/rust/timestamp_50k',
    ],
    'oniguruma': [
        *[f'regex_tasks/rust/{name}' for name in REGEX_TASKS_ONIGURUMA],
        'oniguruma_features/rust/atomic_possessive_strings',
        'oniguruma_features/rust/subexp_call_balanced',
        'oniguruma_features/rust/absent_comments',
        'oniguruma_features/rust/conditional_brackets',
        'oniguruma_features/rust/backref_ignorecase',
        'oniguruma_features/rust/lookbehind_alternation',
    ],
    # One search on a short string, and compiling a pattern: mostly call cost.
    'micro': [
        'single_pattern/rust/literal_exact',
        'single_pattern/rust/quantifier_greedy',
        'single_pattern/rust/alternation_2_branch',
        'single_pattern/rust/alternation_10_branch',
        'single_pattern/rust/case_insensitive_phrase',
        'single_pattern/rust/named_capture_date',
        'single_pattern/rust/unicode_greek',
        'single_pattern/rust/lookaround_combined',
        'single_pattern/rust/backref_simple',
        'text_scanning/regset_position_lead_rust',
        'compilation/rust/literal',
        'compilation/rust/named_capture',
        'compilation/rust/lookbehind',
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
        # Recorded with benches/shiki_js/capture.mjs; every engine runs them.
        'c_scanner/document',
        'c_scanner/group_12',
        'php_scanner/document',
        'php_scanner/group_13',
    ],
}
# Criterion group prefix -> bench target; everything else is battle_bench.
BENCHES = {'cpp_scanner': 'cpp_scanner_bench', 'java_scanner': 'java_scanner_bench',
           'scss_scanner': 'scss_scanner_bench', 'c_scanner': 'shiki_scanner_bench',
           'php_scanner': 'shiki_scanner_bench'}
ENGINES = ('rust', 'c', 'onigmo', 'pcre2_jit', 'pcre2', 'fancy_regex', 'fancy_regex_seek', 'fancy_regex_set',
           'regex', 'shiki_js')
LABELS = {'rust': 'Ferroni', 'c': 'C', 'onigmo': 'Onigmo', 'pcre2_jit': 'PCRE2 JIT', 'pcre2': 'PCRE2',
          'fancy_regex': 'fancy-regex', 'fancy_regex_seek': 'fancy-regex (seek)',
          'fancy_regex_set': 'fancy-regex (RegexSet)', 'regex': 'regex', 'shiki_js': 'Shiki JS'}
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
    # Columns only for the engines that ran or were reported in these results,
    # so a partial run (--engines) does not print empty columns.
    case_names = [case for cases in results.values() for case in cases]
    timed = {engine for cases in results.values() for result in cases.values() for engine in result['timing']}
    engines = [engine for engine in ENGINES if engine != 'rust' and (
        engine in timed or any(variant(case, engine) in notes['unsupported'] for case in case_names))]
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
    # Ferroni always runs: every ratio is taken against it.
    wanted_engines = {*args.engines, 'rust'}
    onigmo_dir = Path(os.environ.get('FERRONI_ONIGMO_DIR', ROOT / '.cache/upstream/onigmo-ruby'))
    onigmo = not args.no_onigmo and 'onigmo' in wanted_engines
    if onigmo and not (onigmo_dir / 'regexec.c').is_file():
        parser.error('Onigmo sources are missing: run scripts/prepare-onigmo-sources.sh or pass --no-onigmo')
    use_shiki_js = not args.no_shiki_js and 'shiki_js' in wanted_engines
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
    wanted = {variant(case, engine) for cases in selected.values() for case in cases for engine in wanted_engines}
    available &= wanted
    # One validation pass of every selected case before any timing. Setup
    # reports which engines could not reproduce a case; the benches report
    # every engine they validate, so keep the notes of the engines that run.
    notes = {'unsupported': {}, 'equivalent': {}}
    for bench, binary in binaries.items():
        ids = [variant(case, engine) for cases in selected.values() for case in cases
               if bench_of(case) == bench for engine in ENGINES if variant(case, engine) in available]
        log = run_bench(binary, ['^(' + '|'.join(map(re.escape, ids)) + ')$', '--test'], out, f'validate-{bench}.log')
        for kind, found in engine_notes(log).items():
            notes[kind].update({bench_id: note for bench_id, note in found.items() if bench_id in wanted})
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


# The README and home page condense a run into three workloads. Highlighting
# needs every grammar: a highlighter that cannot load one is not a candidate.
# Searches may miss a case; the figure is then taken over the cases the engine
# ran, and the summary marks it with a footnote instead of spelling out counts.
WORKLOADS = [
    {'id': 'shared', 'label': 'Everyday patterns', 'short': 'Everyday patterns', 'unit': 'tasks',
     'require_all': False,
     'detail': 'search and extraction in HTML, logs, chat with emoji, Markdown, JSON, CSV and source code, '
               'in syntax the regex crate also runs',
     'cases': CASES['shared']},
    {'id': 'oniguruma', 'label': 'Advanced patterns', 'short': 'Advanced patterns', 'unit': 'tasks',
     'require_all': False,
     'detail': 'look-around, backreferences, possessive groups, subexpression calls, absent expressions '
               'and grapheme clusters',
     'cases': CASES['oniguruma']},
    {'id': 'highlighting', 'label': 'Syntax highlighting', 'short': 'Syntax highlighting', 'unit': 'documents',
     'require_all': True,
     'detail': 'the scanner calls Shiki makes for whole C, Java and PHP documents, in grammars every engine runs',
     'cases': ['c_scanner/document', 'java_scanner/document', 'php_scanner/document']},
]
# The footnotes under the summary table, in the README and on the home page.
PARTIAL_NOTE = ('Ran only some of the tasks: the engine rejects a pattern or finds different matches. '
                'The figure covers the tasks it ran.')
ABSENT_NOTE = 'Not measured: the engine lacks the syntax, or the multi-pattern API the workload needs.'
FIGURE_ENGINES = ('c', 'shiki_js', 'onigmo', 'pcre2', 'pcre2_jit', 'fancy_regex', 'regex')
FIGURE_LABELS = {**LABELS, 'c': 'Oniguruma (C)'}
HOST_LABELS = {'macos-arm64': 'macOS arm64', 'linux-x86-64': 'Linux x86-64'}
GRAMMARS = {'cpp_scanner': 'C++', 'java_scanner': 'Java', 'scss_scanner': 'SCSS', 'c_scanner': 'C',
            'php_scanner': 'PHP'}


def factor_text(factor):
    """One factor as the summary states it: how many times faster or slower."""
    magnitude = factor if factor >= 1 else 1 / factor
    return f'{magnitude:.0f}×' if magnitude >= 10 else f'{magnitude:.1f}×'


def load_runs(directories):
    """The measurements of the published runs, in order, per runner profile.

    Every run must cover both runner profiles with the same case sets. A later
    run's case sets replace the earlier run's, notes included. Returns the
    hosts (host record, mean ns per case and engine, unsupported notes) and
    one record per run: its directory, commit, URL, date and case sets."""
    hosts, runs = {}, []
    for directory in directories:
        directory = Path(directory)
        run = {'data': directory.as_posix(), 'commits': set(), 'urls': set(), 'sets': {}}
        date = re.search(r'(\d{4}-\d{2}-\d{2})', directory.name)
        run['measured'] = date.group(1) if date else None
        for path in sorted(directory.rglob('measurements.json')):
            data = json.loads(path.read_text())
            host = data['host']
            entry = hosts.setdefault(host['runner_profile'], {'host': host, 'sets': {}})
            for case_set, cases in data['results'].items():
                timing = {case: {engine: row['mean_ns'] for engine, row in result['timing'].items()}
                          for case, result in cases.items()}
                ids = {variant(case, engine) for case in cases for engine in ENGINES}
                entry['sets'][case_set] = {
                    'timing': timing,
                    'unsupported': {i: note for i, note in data['notes']['unsupported'].items() if i in ids},
                }
                run['sets'].setdefault(host['runner_profile'], set()).add(case_set)
            run['commits'].add(data['build']['git_commit'])
            run['urls'].add(host['run_url'])
        if (set(run['sets']) != set(HOST_LABELS) or len({frozenset(s) for s in run['sets'].values()}) != 1
                or len(run['commits']) != 1 or len(run['urls']) != 1):
            raise SystemExit(f'{directory}: a run must cover both runner profiles with the same case sets')
        case_sets = sorted(next(iter(run['sets'].values())), key=list(CASES).index)
        runs.append({'data': run['data'], 'commit': run['commits'].pop(), 'run': run['urls'].pop(),
                     'measured': run['measured'], 'cases': case_sets})
    for entry in hosts.values():
        entry['timing'] = {case: timing for s in entry['sets'].values() for case, timing in s['timing'].items()}
        entry['unsupported'] = {i: note for s in entry['sets'].values() for i, note in s['unsupported'].items()}
    return hosts, runs


# fancy-regex runs in three configurations. The summary quotes the fastest one
# per workload and names it in a footnote; the comparison page shows all three.
FANCY_REGEX = {'fancy_regex': 'default', 'fancy_regex_seek': 'seek mode', 'fancy_regex_set': 'RegexSet'}


def configuration_note(cells):
    """The footnote naming fancy-regex's quoted configuration per workload."""
    by_configuration = {}
    for workload in WORKLOADS:
        cell = cells[workload['id']]
        if 'configuration' in cell:
            by_configuration.setdefault(cell['configuration'], []).append(workload['label'].lower())
    parts = []
    for configuration, labels in by_configuration.items():
        joined = labels[0] if len(labels) == 1 else ', '.join(labels[:-1]) + ' and ' + labels[-1]
        parts.append(f'{configuration} for {joined}')
    return 'fancy-regex is quoted in its fastest configuration per column: ' + '; '.join(parts) + '.'


def figures(args):
    """Geometric mean of (engine time / Ferroni time) per workload and host."""
    hosts, runs = load_runs(args.runs)
    unsupported = {i: note for entry in hosts.values() for i, note in entry['unsupported'].items()}
    order = list(HOST_LABELS)

    def figure_cell(engine, workload):
        per_host = {}
        for host_id in order:
            timing = hosts[host_id]['timing']
            ratios = [timing[case][engine] / timing[case]['rust'] for case in workload['cases']
                      if engine in timing[case]]
            per_host[host_id] = (math.exp(sum(map(math.log, ratios)) / len(ratios)) if ratios else None,
                                 len(ratios))
        runs_cases = per_host[order[0]][1]
        total = len(workload['cases'])
        missing = [case for case in workload['cases'] if variant(case, engine) in unsupported]
        cell = {'cases': runs_cases, 'of': total}
        if runs_cases == 0:
            cell['text'] = 'n/a' if missing else '–'
            if missing:
                cell['note'] = 'cannot run these cases'
        elif workload['require_all'] and runs_cases < total:
            grammar_notes = []
            for case in missing:
                reason = unsupported[variant(case, engine)]['reason']
                verb = 'rejects' if reason.startswith(('scanner', 'compile')) else 'differs on'
                grammar_notes.append(f'{verb} {GRAMMARS[case.split("/")[0]]}')
            cell.update(text='n/a', note=', '.join(grammar_notes),
                        grammars=[GRAMMARS[case.split('/')[0]] for case in missing])
        else:
            cell['factors'] = {host_id: round(per_host[host_id][0], 3) for host_id in order}
            # One figure per cell: the geometric mean over both hosts. The
            # per-host values stay in `factors` and on the comparison page.
            combined = math.exp(sum(math.log(per_host[host_id][0]) for host_id in order) / len(order))
            cell['factor'] = round(combined, 3)
            cell['text'] = factor_text(combined)
            cell['direction'] = 'faster' if combined >= 1 else 'slower'
        return cell

    engines = []
    for engine in FIGURE_ENGINES:
        cells = {}
        for workload in WORKLOADS:
            if engine == 'fancy_regex':
                # The fastest configuration: the lowest ratio to Ferroni among
                # those that ran the workload, the default where none did.
                candidates = {name: figure_cell(name, workload) for name in FANCY_REGEX}
                measured = {name: cell for name, cell in candidates.items() if 'factor' in cell}
                name = min(measured, key=lambda n: measured[n]['factor']) if measured else 'fancy_regex'
                cells[workload['id']] = candidates[name] | ({'configuration': FANCY_REGEX[name]} if measured else {})
            else:
                cells[workload['id']] = figure_cell(engine, workload)
        engines.append({'id': engine, 'label': FIGURE_LABELS[engine], 'cells': cells})
    fancy = next(engine for engine in engines if engine['id'] == 'fancy_regex')
    latest = runs[-1]
    out = {
        'measured': latest['measured'],
        'commit': latest['commit'],
        'run': latest['run'],
        'data': latest['data'],
        # Every run the figures draw on, oldest first; a later run's case sets
        # replace the earlier run's. The fields above are the latest run's.
        'runs': runs,
        'hosts': [{'id': host_id, 'label': HOST_LABELS[host_id], 'machine': hosts[host_id]['host']['cpu'],
                   'cpus': hosts[host_id]['host']['cpus'], 'runner': hosts[host_id]['host']['runner_label']}
                  for host_id in order],
        'workloads': [{key: workload[key] for key in ('id', 'label', 'short', 'unit', 'detail')}
                      | {'cases': len(workload['cases'])}
                      for workload in WORKLOADS],
        'notes': {'partial': PARTIAL_NOTE, 'absent': ABSENT_NOTE,
                  'configurations': configuration_note(fancy['cells'])},
        'engines': engines,
    }
    args.output.write_text(json.dumps(out, indent=2, ensure_ascii=False) + '\n')
    print(readme_table(out))


def readme_cell(cell):
    """A README cell: "2.2× faster", with a footnote mark when cases were left out."""
    if 'factor' not in cell:
        return cell['text'] + (f' ({cell["note"]})' if cell.get('note') else '')
    mark = '\\*' if cell['cases'] < cell['of'] else ''
    return f'{cell["text"]} {cell["direction"]}{mark}'


def readme_table(figures_data):
    """The README block; docs/scripts/check-benchmark-claims.mjs compares it."""
    workloads = figures_data['workloads']
    engines = figures_data['engines']
    lines = ['| Ferroni compared with | ' + ' | '.join(w['label'] for w in workloads) + ' |',
             '| --- |' + ' ---: |' * len(workloads)]
    for engine in engines:
        cells = [readme_cell(engine['cells'][workload['id']]) for workload in workloads]
        lines.append(f'| {engine["label"]} | ' + ' | '.join(cells) + ' |')
    lines.append('')
    for workload in workloads:
        lines.append(f'- **{workload["label"]}** ({workload["cases"]} {workload["unit"]}): {workload["detail"]}.')
    cells = [engine['cells'][workload['id']] for engine in engines for workload in workloads]
    footnotes = []
    if any('factor' in cell and cell['cases'] < cell['of'] for cell in cells):
        footnotes.append(f'\\* {figures_data["notes"]["partial"]}')
    if any(cell['text'] == '–' for cell in cells):
        footnotes.append(f'– {figures_data["notes"]["absent"]}')
    if figures_data['notes'].get('configurations'):
        footnotes.append(figures_data['notes']['configurations'])
    for footnote in footnotes:
        lines += ['', footnote]
    return '\n'.join(lines)


DOC_TABLES = [
    ('Text processing, shared syntax', CASES['shared']),
    ('Text processing, Oniguruma syntax', CASES['oniguruma']),
    ('Highlighting, portable grammars', [case for case in CASES['textmate']
                                         if case.split('/')[0] in ('c_scanner', 'java_scanner', 'php_scanner')]),
    ('Highlighting, grammars with Oniguruma-only syntax', [case for case in CASES['textmate']
                                                          if case.split('/')[0] in ('cpp_scanner', 'scss_scanner')]),
    ('Grammar scanners', [case for case in CASES['textmate'] if case.startswith('scanner_')]),
    ('Short searches and compilation', CASES['micro']),
]
DOC_LANGUAGES = {'ts': 'TypeScript', 'css': 'CSS', 'rust': 'Rust'}


def doc_label(case):
    group, _, rest = case.replace('/rust/', '/').removesuffix('_rust').partition('/')
    if group == 'compilation':
        return f'compile `{rest}`'
    if group == 'scanner_highlighting':
        language, _, what = rest.split('_', 2)
        return f'{DOC_LANGUAGES[language]} grammar, ' + {'compile': 'compile', 'tokenize': 're-scan line'}[what]
    if group == 'scanner_documents':
        return f'{DOC_LANGUAGES[rest.split("_")[0]]} document, {rest.split("_")[-2]} lines'
    if group in GRAMMARS:
        return f'{GRAMMARS[group]} replay, ' + ('document' if rest == 'document' else 'scanner ' + rest.removeprefix('group_'))
    return f'`{rest}`'


def tables(args):
    """Per-case tables, Apple Silicon above x86-64 in every cell."""
    loaded, _ = load_runs(args.runs)
    hosts = {host_id: entry['timing'] for host_id, entry in loaded.items()}
    unsupported = {i: note for entry in loaded.values() for i, note in entry['unsupported'].items()}
    order = list(HOST_LABELS)

    def time(value):
        for unit, scale in (('s', 1e9), ('ms', 1e6), ('µs', 1e3)):
            if value >= scale:
                return f'{value / scale:.3g}\u00a0{unit}'
        return f'{value:.3g}\u00a0ns'

    others = [engine for engine in ENGINES if engine != 'rust']
    for title, cases in DOC_TABLES:
        used = [engine for engine in others
                if any(engine in hosts[host][case] or variant(case, engine) in unsupported
                       for host in order for case in cases)]
        print(f'### {title}\n')
        print('| Case | Ferroni | ' + ' | '.join(FIGURE_LABELS[engine] for engine in used) + ' |')
        print('| --- | ---: |' + ' ---: |' * len(used))
        for case in cases:
            ferroni = [hosts[host][case]['rust'] for host in order]
            cells = []
            for engine in used:
                values = []
                for host, base in zip(order, ferroni):
                    if engine in hosts[host][case]:
                        values.append(f'{hosts[host][case][engine] / base:.2f}')
                    elif variant(case, engine) in unsupported:
                        values.append('n/a')
                    else:
                        values.append('–')
                cells.append(values[0] if values[0] == values[1] and values[0] in ('–', 'n/a')
                             else '<br />'.join(values))
            print(f'| {doc_label(case)} | {time(ferroni[0])}<br />{time(ferroni[1])} | ' + ' | '.join(cells) + ' |')
        print()
    selected = {variant(case, engine) for _, cases in DOC_TABLES for case in cases for engine in ENGINES}
    print('### Cases an engine cannot run\n')
    print('| Case | Engine | Reason |')
    print('| --- | --- | --- |')
    for bench_id, note in sorted(unsupported.items()):
        if bench_id not in selected:
            continue
        case = next(case for _, cases in DOC_TABLES for case in cases
                    if any(variant(case, engine) == bench_id for engine in ENGINES))
        print(f'| {doc_label(case)} | {FIGURE_LABELS[note["engine"]]} | {short_reason(note["reason"], 140)} |')


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
    run_parser.add_argument('--engines', nargs='+', choices=ENGINES, default=list(ENGINES),
                            help='time only these engines next to Ferroni, which always runs (local runs)')
    run_parser.add_argument('--no-onigmo', action='store_true', help='build without the onigmo feature')
    run_parser.add_argument('--no-shiki-js', action='store_true', help='skip the Node replays of Shiki\'s JS engine')
    run_parser.add_argument('--target-dir', type=Path, default=ROOT / 'target' / 'compare-engines')
    report_parser = commands.add_parser('report', help='merge the measurements of several runs')
    report_parser.add_argument('summary', type=Path)
    report_parser.add_argument('directories', nargs='+', type=Path)
    figures_parser = commands.add_parser('figures', help='condense the published runs for the README and home page')
    figures_parser.add_argument('runs', nargs='+', type=Path, help='run directories, a later one overriding its case sets')
    figures_parser.add_argument('output', type=Path)
    tables_parser = commands.add_parser('tables', help='print the per-case tables of the engine comparison page')
    tables_parser.add_argument('runs', nargs='+', type=Path, help='run directories, a later one overriding its case sets')
    args = parser.parse_args()
    if args.command == 'figures':
        figures(args)
    elif args.command == 'tables':
        tables(args)
    elif args.command == 'run':
        if not 0.5 <= args.measure_seconds <= 30 or not 0.1 <= args.warm_up_seconds <= 5:
            run_parser.error('--measure-seconds must be 0.5-30 and --warm-up-seconds 0.1-5')
        run(args, run_parser)
    else:
        report(args)


if __name__ == '__main__':
    sys.exit(main())
