import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
FERRIKI = Path('/Users/sebastian/Workspace/.codex/dfa-replay-consumer/ferriki')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')
PNPM = '/Users/sebastian/Library/pnpm/.tools/pnpm/10.28.2/bin/pnpm'

def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()

def main():
    assert git(ROOT, 'rev-parse', 'HEAD') == 'adb31da5001459a10d5456209823dbeb8bff38ae'
    assert git(ROOT, 'status', '--porcelain') == ''
    assert git(FERRIKI, 'rev-parse', 'HEAD') == 'e7a4fb17e1eb34b45d4ed1bcb8d36d8de4ce85d8'
    assert git(FERRIKI, 'status', '--porcelain') == ''
    assert subprocess.run(['git','-C',str(FERRIKI),'symbolic-ref','-q','HEAD'],stdout=subprocess.DEVNULL).returncode == 1
    target = OUT / 'validation-retry'
    target.mkdir()
    config = FERRIKI / '.cargo/config.toml'
    assert not config.exists() and not config.parent.exists()
    config.parent.mkdir()
    patch = '[patch.crates-io]\nferroni = { path = "/private/tmp/ferroni-dfa-candidate-walk-20261017" }\n'
    config.write_text(patch)
    env = dict(os.environ, FERRIKI_FERRONI_PATH=str(ROOT), CARGO_TARGET_DIR=str(OUT/'consumer/target'), CARGO_NET_OFFLINE='true')
    jobs = [
        ('current-core-compat', [PNPM, 'run', 'test:ferriki-compat:core']),
        ('current-curated-parity', [PNPM, 'run', 'check:bench-curated']),
        ('current-tiobe-parity', [PNPM, 'run', 'check:bench-tiobe']),
    ]
    records = []
    lock = FERRIKI / 'Cargo.lock'
    original = lock.read_bytes()
    assert original == (OUT/'consumer/original-Cargo.lock').read_bytes()
    try:
        for name, command in jobs:
            print('START ' + name, flush=True)
            with (target / (name+'.log')).open('w') as log:
                result = subprocess.run(command, cwd=FERRIKI/'node', env=env, stdout=log, stderr=subprocess.STDOUT)
            records.append({'name':name,'cwd':str(FERRIKI/'node'),'command':command,'state':'PASSED' if result.returncode==0 else 'FAILED','returncode':result.returncode})
            (target/'summary.json').write_text(json.dumps(records,indent=2)+'\n')
            print(name + ': ' + records[-1]['state'], flush=True)
            assert result.returncode == 0, (target / (name+'.log')).read_text()[-4000:]
    finally:
        changed = lock.read_bytes()
        expected = json.loads((OUT/'consumer/context.json').read_text())['lock_patched_sha256']
        assert changed == original or hashlib.sha256(changed).hexdigest() == expected
        lock.write_bytes(original)
        assert config.read_text() == patch
        config.unlink()
        config.parent.rmdir()
        assert git(ROOT, 'status', '--porcelain') == ''
        assert git(FERRIKI, 'status', '--porcelain') == ''
    receipt = json.loads((OUT/'consumer/candidate-build.json').read_text())
    assert hashlib.sha256((FERRIKI/'node/platforms/darwin-arm64/ferriki.node').read_bytes()).hexdigest() == receipt['binarySha256']
    print('Validated addon is byte-identical to the measured candidate', flush=True)

if __name__ == '__main__':
    main()
