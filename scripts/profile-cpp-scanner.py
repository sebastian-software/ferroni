#!/usr/bin/env python3
"""Build and record a bounded, warmed Ferroni C++ scanner CPU workload."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path, help='artifact prefix')
parser.add_argument('--group', type=int, help='scanner group ID; default: full document')
parser.add_argument('--seconds', type=int, default=20)
parser.add_argument('--sample', action='store_true', help='also collect a 10-second macOS sample')
parser.add_argument('--target-dir', type=Path, default=ROOT / 'target' / 'cpp-profile')
args = parser.parse_args()
if not 15 <= args.seconds <= 300:
    parser.error('--seconds must be between 15 and 300')
if args.sample and sys.platform != 'darwin':
    parser.error('--sample requires macOS; use perf/samply with the example on Linux')
args.output = args.output.resolve()
args.output.parent.mkdir(parents=True, exist_ok=True)
args.target_dir = args.target_dir.resolve()


def output(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


build_env = os.environ.copy()
build_env.update(CARGO_TARGET_DIR=str(args.target_dir),
                 CARGO_PROFILE_RELEASE_DEBUG='line-tables-only',
                 CARGO_PROFILE_RELEASE_STRIP='none')
command = ['cargo', 'build', '--locked', '--release', '--example', 'profile_cpp_scanner']
subprocess.run(command, cwd=ROOT, env=build_env, check=True)
binary = args.target_dir / 'release/examples/profile_cpp_scanner'
source = hashlib.sha256()
for path in sorted((ROOT / 'src').rglob('*.rs')):
    source.update(path.relative_to(ROOT).as_posix().encode() + b'\0' + path.read_bytes())
receipt = {
    'git_commit': output(['git', 'rev-parse', 'HEAD']),
    'git_status': output(['git', 'status', '--porcelain']),
    'rust_source_sha256': source.hexdigest(),
    'fixture_sha256': digest(ROOT / 'benches/cpp_scanner/trace.json'),
    'lockfile_sha256': digest(ROOT / 'Cargo.lock'),
    'binary_sha256': digest(binary),
    'rustc': output(['rustc', '-Vv']),
    'platform': platform.platform(),
    'machine': platform.machine(),
    'build_command': command,
    'build_environment': {k: v for k, v in build_env.items()
                          if k.startswith('CARGO_PROFILE_') or k in
                          ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_TARGET')},
}
run = [str(binary), str(args.seconds)]
if args.group is not None:
    run.append(str(args.group))
receipt['run_command'] = run
process = subprocess.Popen(run, cwd=ROOT, stdout=subprocess.PIPE,
                           stderr=subprocess.PIPE, text=True)
try:
    while True:
        line = process.stderr.readline()
        if not line:
            raise RuntimeError('workload exited before warmup: ' + process.stdout.read())
        print(line, end='', file=sys.stderr)
        if line.startswith('Warm workload ready;'):
            break
    if args.sample:
        sample_path = Path(str(args.output) + '.sample.txt')
        subprocess.run(['/usr/bin/sample', str(process.pid), '10', '1', '-file', str(sample_path)], check=True)
        with gzip.open(str(sample_path) + '.gz', 'wb') as archive:
            archive.write(sample_path.read_bytes())
        sample_path.unlink()
    stdout, stderr = process.communicate(timeout=args.seconds + 30)
    if process.returncode:
        raise RuntimeError(stderr)
    with gzip.open(str(args.output) + '.json.gz', 'wt') as archive:
        json.dump({'build': receipt, 'workload': json.loads(stdout)}, archive, indent=2)
finally:
    if process.poll() is None:
        process.terminate()
        process.communicate(timeout=10)
