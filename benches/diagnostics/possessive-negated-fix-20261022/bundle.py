#!/usr/bin/env python3
"""Bundle retained diagnostic evidence, then verify every archived file hash."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import tarfile

PHASES = ['probe-helper01', 'probe-helper02', 'probe-helper03', 'probe-track001',
          'confirm-helper03', 'control-aa', 'broad', 'probe', 'try02', 'aaend', 'final',
          'pop01', 'sta01', 'cmp04', 'aa64x', 'wide2', 'codegen',
          'codegen-compact3', 'codegen-popleave', 'codegen-stable01', 'codegen-compact45']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    files = []
    for path in args.root.iterdir():
        if path.is_file() and not path.is_symlink() and not path.name.endswith('-draft.rs'):
            if path.suffix in {'.json', '.log', '.txt', '.patch', '.rs', '.py'}:
                if path.name != 'archive-audit.json':
                    files.append(path)
    for name in PHASES:
        directory = args.root / name
        assert directory.is_dir(), directory
        for path in directory.rglob('*'):
            assert not path.is_symlink(), path
            if path.is_file():
                files.append(path)
    files.sort(key=lambda path: path.relative_to(args.root).as_posix())
    manifest = {path.relative_to(args.root).as_posix():
                {'sha256': sha(path.read_bytes()), 'bytes': path.stat().st_size}
                for path in files}
    manifest_data = (json.dumps(manifest, indent=2) + '\n').encode()
    output = args.destination / 'evidence.tar.gz'
    with output.open('wb') as raw:
        with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode='w') as archive:
                for path in files:
                    info = archive.gettarinfo(str(path), path.relative_to(args.root).as_posix())
                    info.uid = info.gid = info.mtime = 0
                    info.uname = info.gname = ''
                    info.mode = 0o644
                    with path.open('rb') as source:
                        archive.addfile(info, source)
                info = tarfile.TarInfo('MANIFEST.json')
                info.size = len(manifest_data)
                info.mode = 0o644
                archive.addfile(info, io.BytesIO(manifest_data))
    with tarfile.open(output, 'r:gz') as archive:
        assert set(archive.getnames()) == set(manifest) | {'MANIFEST.json'}
        retained = json.load(archive.extractfile('MANIFEST.json'))
        assert retained == manifest
        for name, expected in retained.items():
            data = archive.extractfile(name).read()
            assert len(data) == expected['bytes'] and sha(data) == expected['sha256'], name
    digest = sha(output.read_bytes())
    (args.destination / 'evidence.sha256').write_text(digest + '  evidence.tar.gz\n')
    audit = {'files': len(files), 'all_archived_hashes_verified': True,
             'archive_bytes': output.stat().st_size, 'archive_sha256': digest}
    (args.root / 'archive-audit.json').write_text(json.dumps(audit, indent=2) + '\n')
    print(json.dumps(audit, indent=2))


if __name__ == '__main__':
    main()
