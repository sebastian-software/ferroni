import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')
HEAD = 'adb31da5001459a10d5456209823dbeb8bff38ae'
NAMES = ['candidate_calls', 'candidate_memo_hits', 'walks', 'plain_iterations', 'plain_bytes',
         'plain_limit_checks', 'plain_limit_hits', 'admission_entries', 'long_patterns_visited',
         'long_patterns_admitted', 'second_phase_bytes', 'second_phase_limit_checks',
         'second_phase_limit_hits', 'finished_exits', 'bound_exits', 'settled_exits',
         'gave_up_exits', 'asked_exits', 'walked_bytes', 'zero_byte_walks',
         'walks_1_to_16_bytes', 'walks_17_to_64_bytes', 'walks_65_to_256_bytes', 'walks_over_256_bytes']

def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()

def replace_once(text, before, after):
    assert text.count(before) == 1, before
    return text.replace(before, after, 1)

def main():
    assert len(json.loads((OUT / 'measurements.json').read_text())) == 128, 'Finish the fixed timing plan before diagnostic builds'
    assert git('rev-parse', '--path-format=absolute', '--git-common-dir') == '/Users/sebastian/Workspace/ferroni/.git'
    assert git('rev-parse', '--show-toplevel') == str(ROOT)
    assert git('symbolic-ref', '--short', 'HEAD') == 'codex/dfa-candidate-walk'
    assert git('rev-parse', 'HEAD') == HEAD
    assert git('status', '--porcelain') == ''
    paths = ['src/dfa_prefilter.rs', 'src/lib.rs', 'examples/profile_cpp_scanner.rs']
    original = {name: (ROOT / name).read_bytes() for name in paths}
    modified = {name: value.decode() for name, value in original.items()}
    source = modified['src/dfa_prefilter.rs']
    source = replace_once(source, '        self.candidates.clear();\n        self.walked_at = Some(at);',
                          '        diagnostic_add(0, 1);\n        self.candidates.clear();\n        self.walked_at = Some(at);')
    source = replace_once(source, '            if let Some((memo, some)) = self.candidate_memo.get(at) {',
                          '            if let Some((memo, some)) = self.candidate_memo.get(at) {\n                diagnostic_add(1, 1);')
    start = source.index('    fn walk(\n')
    end = source.index('    /// The overlapping DFA gave up', start)
    walk = source[start:end]
    walk = replace_once(walk, '        let end = haystack.len();', '        diagnostic_add(2, 1);\n        let end = haystack.len();')
    walk = replace_once(walk, '        let plain_end = cut.min(ask_i);\n        loop {', '        let plain_end = cut.min(ask_i);\n        loop {\n            diagnostic_add(3, 1);')
    walk = replace_once(walk, '            if *i >= plain_end {', '            diagnostic_add(5, 1);\n            if *i >= plain_end {\n                diagnostic_add(6, 1);')
    walk = replace_once(walk, '        *stamp = stamp.wrapping_add(1);', '        diagnostic_add(7, 1);\n        *stamp = stamp.wrapping_add(1);')
    walk = replace_once(walk, '        for &pid in long.iter() {', '        for &pid in long.iter() {\n            diagnostic_add(8, 1);')
    walk = replace_once(walk, '            if admits(entries[pid as usize], at) {', '            if admits(entries[pid as usize], at) {\n                diagnostic_add(9, 1);')
    plain, second = walk.split('        // On, until every admissible long seek has been recorded.\n')
    plain = replace_once(plain, '            *i += 1;', '            *i += 1;\n            diagnostic_add(4, 1);')
    second = replace_once(second, '            *i += 1;', '            *i += 1;\n            diagnostic_add(10, 1);')
    second = replace_once(second, '            if *i == cut {', '            diagnostic_add(11, 1);\n            if *i == cut {\n                diagnostic_add(12, 1);')
    walk = plain + '        // On, until every admissible long seek has been recorded.\n' + second
    walk, replacements = re.subn(r'return \((ScanEnd::\w+), (\$asked|true|false)\);', r'return diagnostic_end(\1, \2, *i - at);', walk)
    assert replacements >= 10
    source = source[:start] + walk + source[end:]
    source += '''

// Diagnostic-only instrumentation, retained as a patch, never shipped.
thread_local! {
    static WALK_DIAGNOSTIC: std::cell::Cell<[u64; 24]> = const { std::cell::Cell::new([0; 24]) };
}

fn diagnostic_add(index: usize, count: u64) {
    WALK_DIAGNOSTIC.with(|cell| {
        let mut counts = cell.get();
        counts[index] += count;
        cell.set(counts);
    });
}

fn diagnostic_end(end: ScanEnd, asked: bool, walked: usize) -> (ScanEnd, bool) {
    diagnostic_add(match end { ScanEnd::Finished => 13, ScanEnd::Bound => 14, ScanEnd::Settled => 15, ScanEnd::GaveUp => 16 }, 1);
    diagnostic_add(17, u64::from(asked));
    diagnostic_add(18, walked as u64);
    diagnostic_add(match walked { 0 => 19, 1..=16 => 20, 17..=64 => 21, 65..=256 => 22, _ => 23 }, 1);
    (end, asked)
}

pub(crate) fn diagnostic_reset() { WALK_DIAGNOSTIC.with(|cell| cell.set([0; 24])); }
pub(crate) fn diagnostic_counts() -> [u64; 24] { WALK_DIAGNOSTIC.with(std::cell::Cell::get) }
'''
    modified['src/dfa_prefilter.rs'] = source
    modified['src/lib.rs'] += '''

/// Diagnostic build only; not part of the submitted library.
#[doc(hidden)]
pub fn __walk_diagnostic_reset() { dfa_prefilter::diagnostic_reset(); }
/// Diagnostic build only; not part of the submitted library.
#[doc(hidden)]
pub fn __walk_diagnostic_counts() -> [u64; 24] { dfa_prefilter::diagnostic_counts() }
'''
    example = modified['examples/profile_cpp_scanner.rs']
    example = replace_once(example, '    let wall = Instant::now();', '    ferroni::__walk_diagnostic_reset();\n    let wall = Instant::now();')
    example = replace_once(example, '"ferroni_version": env!("CARGO_PKG_VERSION"),', '"ferroni_version": env!("CARGO_PKG_VERSION"), "walk_counts": ferroni::__walk_diagnostic_counts(),')
    modified['examples/profile_cpp_scanner.rs'] = example
    diagnostic = OUT / 'diagnostics'
    diagnostic.mkdir()
    env = dict(os.environ, CARGO_TARGET_DIR=str(OUT / 'diagnostic-target'), FERRONI_ONIGURUMA_DIR='/Users/sebastian/Workspace/ferroni/.cache/upstream/oniguruma-orig')
    try:
        for name in paths:
            (ROOT / name).write_text(modified[name])
        (diagnostic / 'instrumentation.patch').write_text(git('diff', '--', *paths) + '\n')
        command = ['cargo', 'build', '--offline', '--locked', '--release', '--features', 'ffi', '--example', 'profile_cpp_scanner']
        print('Building counters in a separate diagnostic binary', flush=True)
        with (diagnostic / 'build.log').open('w') as log:
            subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = OUT / 'diagnostic-target/release/examples/profile_cpp_scanner'
        (diagnostic / 'build.json').write_text(json.dumps({'head': HEAD, 'command': command, 'counter_names': NAMES,
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'timing_claim': False}, indent=2) + '\n')
    finally:
        for name in paths:
            assert (ROOT / name).read_bytes() in [original[name], modified[name].encode()]
            (ROOT / name).write_bytes(original[name])
        assert git('rev-parse', 'HEAD') == HEAD and git('status', '--porcelain') == ''
    rows = []
    for language, group in [('cpp', 'all'), ('cpp', '78'), ('tsx', 'all'), ('tsx', '2'), ('tsx', '1'), ('tsx', '29')]:
        case = language + '_' + group
        command = [str(binary), '1', group, str(ROOT / f'benches/{language}_scanner/trace.json')]
        result = subprocess.run(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=60)
        (diagnostic / f'{case}.json').write_text(result.stdout)
        (diagnostic / f'{case}.log').write_text(result.stderr)
        data = json.loads(result.stdout)
        repetitions = len(data['timings_ns'])
        counts = dict(zip(NAMES, data['walk_counts']))
        row = {'case': case, 'replays': repetitions, 'calls_per_replay': data['calls_per_replay'],
               'counts': counts, 'counts_per_replay': {k: v / repetitions for k, v in counts.items()}}
        rows.append(row)
        print(case, json.dumps(row['counts_per_replay']), flush=True)
    (diagnostic / 'summary.json').write_text(json.dumps(rows, indent=2) + '\n')

if __name__ == '__main__':
    main()
