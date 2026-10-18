import gzip
import json
from pathlib import Path
import subprocess
import time

ROOT = Path('/private/tmp/ferroni-dfa-candidate-walk-20261017')
OUT = Path('/private/tmp/ferroni-dfa-replay-20261017')

def main():
    assert len(json.loads((OUT / 'measurements.json').read_text())) == 128
    profiles = OUT / 'profiles'
    profiles.mkdir()
    for language in ['cpp', 'tsx']:
        for variant in ['baseline', 'candidate']:
            name = language + '-' + variant
            profile = profiles / (name + '.json.gz')
            driver = OUT / 'binaries' / variant / 'profile_cpp_scanner'
            command = ['samply', 'record', '--save-only', '--unstable-presymbolicate', '-o', str(profile), '--', str(driver), '20', 'all', str(ROOT / f'benches/{language}_scanner/trace.json')]
            print(f'Profiling {name} for 20 steady seconds', flush=True)
            process = subprocess.Popen(command, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stderr = []
            steady = None
            try:
                while True:
                    line = process.stderr.readline()
                    if not line:
                        raise RuntimeError('Profiler stopped before the steady marker: ' + ''.join(stderr))
                    stderr.append(line)
                    if 'Warm workload ready;' in line:
                        steady = time.time() * 1000
                        break
                stdout, remaining = process.communicate(timeout=90)
                stderr.append(remaining)
                assert process.returncode == 0, ''.join(stderr)
                with gzip.open(profiles / (name + '-driver-output.json.gz'), 'wt') as archive:
                    archive.write(stdout)
                (profiles / (name + '.log')).write_text(''.join(stderr))
                (profiles / (name + '-run.json')).write_text(json.dumps({'command': command, 'steady_from_epoch_ms': steady,
                    'analysis_window_ms': 19000, 'purpose': 'diagnostic CPU attribution, not a speed measurement'}, indent=2) + '\n')
                analysis = ['node', '/private/tmp/ferriki-profile-after-20261017/analyze-profile.mjs', str(profile), '--since', str(steady), '--top', '12', '--focus', 'SetPrefilter.*(candidates_at|walk|scan_candidates)']
                with (profiles / (name + '-summary.txt')).open('w') as log:
                    subprocess.run(analysis, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.communicate(timeout=10)
            print(name + ' retained', flush=True)

if __name__ == '__main__':
    main()
