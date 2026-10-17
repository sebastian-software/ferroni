import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

FERRIKI = Path('/Users/sebastian/Workspace/.codex/dfa-replay-consumer/ferriki')
FERRONI = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017/consumer')
PNPM = '/Users/sebastian/Library/pnpm/.tools/pnpm/10.28.2/bin/pnpm'
FERRIKI_HEAD = 'e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8'
FERRONI_HEAD = 'adb31da5001459a10d5456209823dbeb8bff38ae'
FERRONI_BASE = '92c2363ed48c99f281805b6cfa4278126588a4c5'

def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()

def run(command, label, env=None, cwd=None):
    print(label, flush=True)
    with (OUT / f'{label}.log').open('w') as log:
        subprocess.run(command, cwd=cwd or FERRIKI / 'node', env=env, stdout=log, stderr=subprocess.STDOUT, check=True)

def main():
    assert git(FERRIKI, 'rev-parse', '--path-format=absolute', '--git-common-dir') == '/Users/sebastian/Workspace/ferriki/.git'
    assert git(FERRIKI, 'rev-parse', '--show-toplevel') == str(FERRIKI)
    assert subprocess.run(['git', '-C', str(FERRIKI), 'symbolic-ref', '-q', 'HEAD'], stdout=subprocess.DEVNULL).returncode == 1
    assert git(FERRIKI, 'rev-parse', 'HEAD') == FERRIKI_HEAD and git(FERRIKI, 'status', '--porcelain') == ''
    assert git(FERRONI, 'rev-parse', '--path-format=absolute', '--git-common-dir') == '/Users/sebastian/Workspace/ferroni/.git'
    assert git(FERRONI, 'symbolic-ref', '--short', 'HEAD') == 'codex/dfa-candidate-walk'
    assert git(FERRONI, 'rev-parse', 'HEAD') == FERRONI_HEAD and git(FERRONI, 'status', '--porcelain') == ''
    OUT.mkdir()
    # Reuse immutable payloads only after checking the catalog identity. The
    # repository materializer validates every payload hash during each build.
    existing = Path('/Users/sebastian/Workspace/ferriki')
    assert (existing / 'assets/catalog.json').read_bytes() == (FERRIKI / 'assets/catalog.json').read_bytes()
    shutil.copytree(existing / 'assets/standard', FERRIKI / 'assets/standard', dirs_exist_ok=True)
    env = dict(os.environ, FERRIKI_FERRONI_PATH=str(FERRONI), CARGO_TARGET_DIR=str(OUT / 'target'), CARGO_NET_OFFLINE='true')
    run([PNPM, 'install', '--frozen-lockfile', '--offline'], 'install', env)
    source = FERRONI / 'src/dfa_prefilter.rs'
    candidate = source.read_bytes()
    baseline = subprocess.check_output(['git', '-C', str(FERRONI), 'show', FERRONI_BASE + ':src/dfa_prefilter.rs'])
    lock = FERRIKI / 'Cargo.lock'
    original_lock = lock.read_bytes()
    (OUT / 'original-Cargo.lock').write_bytes(original_lock)
    patched_lock = None
    try:
        for variant, contents in [('baseline', baseline), ('candidate', candidate)]:
            assert git(FERRIKI, 'rev-parse', 'HEAD') == FERRIKI_HEAD
            assert git(FERRONI, 'rev-parse', 'HEAD') == FERRONI_HEAD
            source.write_bytes(contents)
            run(['node', 'ferriki/scripts/build-native.mjs'], variant + '-native-build', env)
            if variant == 'baseline':
                patched_lock = lock.read_bytes()
                run([PNPM, '-C', 'ferriki', 'build'], 'facade-build', env)
            else:
                assert lock.read_bytes() == patched_lock
            run(['python3', '/private/tmp/ferriki-independent-perf-20261017/save-package.py', str(FERRIKI), str(OUT), variant], variant + '-save-package', env)
    finally:
        assert source.read_bytes() in [baseline, candidate]
        source.write_bytes(candidate)
        if patched_lock is not None:
            assert lock.read_bytes() == patched_lock
        lock.write_bytes(original_lock)
        assert git(FERRONI, 'rev-parse', 'HEAD') == FERRONI_HEAD and git(FERRONI, 'status', '--porcelain') == ''
        assert git(FERRIKI, 'rev-parse', 'HEAD') == FERRIKI_HEAD and git(FERRIKI, 'status', '--porcelain') == ''
    a, b = [json.loads((OUT / f'{variant}-build.json').read_text()) for variant in ['baseline', 'candidate']]
    assert a['rustSourceSha256'] == b['rustSourceSha256']
    assert a['cargoLockSha256'] == b['cargoLockSha256']
    assert a['ferroni']['sourceSha256'] != b['ferroni']['sourceSha256']
    assert a['binarySha256'] != b['binarySha256']
    (OUT / 'context.json').write_text(json.dumps({'ferriki_commit': FERRIKI_HEAD, 'ferroni_baseline': FERRONI_BASE,
        'ferroni_candidate': FERRONI_HEAD, 'lock_original_sha256': hashlib.sha256(original_lock).hexdigest(),
        'lock_patched_sha256': hashlib.sha256(patched_lock).hexdigest(), 'root': str(FERRIKI),
        'same_ferriki_rust_source': True, 'same_patched_lockfile': True}, indent=2) + '\n')
    print('Current-main packages retained; both source worktrees are clean', flush=True)

if __name__ == '__main__':
    main()
