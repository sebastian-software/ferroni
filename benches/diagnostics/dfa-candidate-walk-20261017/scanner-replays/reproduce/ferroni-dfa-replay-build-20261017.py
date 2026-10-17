import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')
HEAD = 'adb31da5001459a10d5456209823dbeb8bff38ae'
BASE = '92c2363ed48c99f281805b6cfa4278126588a4c5'

def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()

def digest(data):
    return hashlib.sha256(data).hexdigest()

assert git('rev-parse', '--path-format=absolute', '--git-common-dir') == '/Users/sebastian/Workspace/ferroni/.git'
assert git('rev-parse', '--show-toplevel') == str(ROOT)
assert git('symbolic-ref', '--short', 'HEAD') == 'codex/dfa-candidate-walk'
assert git('rev-parse', 'HEAD') == HEAD
assert git('status', '--porcelain') == ''
if OUT.exists():
    assert not any(OUT.iterdir()), 'Only the empty output directory from the failed metadata probe may be reused'
else:
    OUT.mkdir()
source = ROOT / 'src/dfa_prefilter.rs'
candidate = source.read_bytes()
baseline = subprocess.check_output(['git', '-C', str(ROOT), 'show', BASE + ':src/dfa_prefilter.rs'])
env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(OUT / 'target'), FERRONI_ONIGURUMA_DIR='/Users/sebastian/Workspace/ferroni/.cache/upstream/oniguruma-orig')
assert not any(k in env for k in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'FERRONI_BENCH_PREFILTER'])
common = {
    'head': HEAD, 'baseline': BASE, 'root': str(ROOT),
    'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
    'cargo': subprocess.check_output(['cargo', '-V'], text=True).strip(),
    'platform': platform.platform(), 'machine': platform.machine(),
    'hardware': subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip(),
    'lock_sha256': digest((ROOT / 'Cargo.lock').read_bytes()),
    'traces': {lang: digest((ROOT / f'benches/{lang}_scanner/trace.json').read_bytes()) for lang in ['cpp', 'tsx']},
    'environment': {k: v for k, v in env.items() if k.startswith('CARGO_PROFILE_') or k in ['CARGO_TARGET_DIR', 'FERRONI_ONIGURUMA_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']},
}
(OUT / 'build-context.json').write_text(json.dumps(common, indent=2) + '\n')
try:
    for name, contents in [('baseline', baseline), ('candidate', candidate)]:
        assert git('rev-parse', 'HEAD') == HEAD
        source.write_bytes(contents)
        dest = OUT / 'binaries' / name
        dest.mkdir(parents=True)
        command = ['cargo', 'bench', '--offline', '--locked', '--no-run', '--features', 'ffi', '--bench', 'cpp_scanner_bench', '--bench', 'ferriki_scanner_bench', '--message-format=json-render-diagnostics']
        print(f'{name}: building ordinary benchmark binaries', flush=True)
        with (OUT / f'{name}-bench-build.jsonl').open('w') as stdout, (OUT / f'{name}-bench-build.log').open('w') as stderr:
            subprocess.run(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr, check=True)
        executables = {}
        for line in (OUT / f'{name}-bench-build.jsonl').read_text().splitlines():
            artifact = json.loads(line)
            if artifact.get('reason') == 'compiler-artifact' and artifact.get('executable'):
                target = artifact['target']['name']
                if target in ['cpp_scanner_bench', 'ferriki_scanner_bench']:
                    executable = Path(artifact['executable'])
                    shutil.copy2(executable, dest / target)
                    executables[target] = digest(executable.read_bytes())
        assert len(executables) == 2
        profile_command = ['cargo', 'build', '--offline', '--locked', '--release', '--features', 'ffi', '--example', 'profile_cpp_scanner']
        print(f'{name}: building bounded profiling driver', flush=True)
        with (OUT / f'{name}-driver-build.log').open('w') as log:
            subprocess.run(profile_command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        driver = OUT / 'target/release/examples/profile_cpp_scanner'
        shutil.copy2(driver, dest / 'profile_cpp_scanner')
        executables['profile_cpp_scanner'] = digest(driver.read_bytes())
        rust_source = hashlib.sha256()
        for path in sorted((ROOT / 'src').rglob('*.rs')):
            rust_source.update(path.relative_to(ROOT).as_posix().encode() + b'\0' + path.read_bytes())
        receipt = dict(common, variant=name, source_sha256=rust_source.hexdigest(), dfa_prefilter_sha256=digest(contents), executables=executables, commands=[command, profile_command], built_at=time.time())
        (OUT / f'{name}-build.json').write_text(json.dumps(receipt, indent=2) + '\n')
        print(f'{name}: binaries retained', flush=True)
finally:
    assert source.read_bytes() in [baseline, candidate]
    source.write_bytes(candidate)
    assert git('rev-parse', 'HEAD') == HEAD
    assert git('status', '--porcelain') == ''
    print('Candidate source restored; worktree clean', flush=True)
