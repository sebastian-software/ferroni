"""Record a warmed native highlighting workload; never mix sampled timings with the baseline."""
import argparse
import pathlib
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('node_root', type=pathlib.Path)
parser.add_argument('language')
parser.add_argument('output', type=pathlib.Path)
parser.add_argument('--api', choices=['html', 'tokens'], default='html')
parser.add_argument('--scopes', action='store_true')
args = parser.parse_args()
if args.scopes and args.api != 'tokens':
    parser.error('--scopes requires --api tokens')
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.with_suffix('.profile.json').open('w') as result, args.output.with_suffix('.profiler.log').open('w') as log:
    child = subprocess.Popen(['node', 'scripts/profile-tiobe.mjs', '--corpus', 'curated', '--language', args.language, '--api', args.api, '--boundary', 'native', '--seconds', '20'] + (['--scopes'] if args.scopes else []), cwd=args.node_root, stdout=result, stderr=subprocess.PIPE, text=True)
    try:
        for line in child.stderr:
            log.write(line)
            log.flush()
            if 'Warm workload ready' in line:
                break
        else:
            raise RuntimeError('Workload exited before the warm marker')
        recording = subprocess.run(['/usr/bin/sample', str(child.pid), '10', '1', '-file', str(args.output)], stdout=log, stderr=log)
        if recording.returncode:
            raise RuntimeError(f'Profiler failed: {recording.returncode}')
        if child.wait(timeout=60):
            raise RuntimeError('Workload failed')
    finally:
        if child.poll() is None:
            child.terminate()
            child.wait(timeout=10)
print(args.output)
