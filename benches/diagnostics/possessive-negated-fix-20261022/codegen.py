#!/usr/bin/env python3
"""Retain Mach-O code hashes and the VM bodies used in the layout investigation."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess


def text_section(binary):
    data = binary.read_bytes()
    assert struct.unpack_from('<I', data)[0] == 0xFEEDFACF
    commands = struct.unpack_from('<I', data, 16)[0]
    cursor = 32
    for _ in range(commands):
        command, size = struct.unpack_from('<II', data, cursor)
        if command == 0x19:
            count = struct.unpack_from('<I', data, cursor + 64)[0]
            for index in range(count):
                section = cursor + 72 + 80 * index
                if data[section:section + 16].rstrip(b'\0') == b'__text':
                    length = struct.unpack_from('<Q', data, section + 40)[0]
                    offset = struct.unpack_from('<I', data, section + 48)[0]
                    return {'size': length, 'sha256': hashlib.sha256(data[offset:offset + length]).hexdigest()}
        cursor += size
    raise ValueError('no __text section')


def inspect(binary, output):
    assembly = subprocess.check_output(['/usr/bin/otool', '-tvV', str(binary)], text=True)
    functions = []
    body = None
    for line in assembly.splitlines():
        if line.startswith('0'):
            if body is not None:
                body.append(line)
        else:
            body = None
            if '_7ferroni7regexec13match_at_impl' in line and line.endswith(':'):
                body = []
                functions.append((line[:-1], body))
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
              'text': text_section(binary), 'vm_functions': []}
    for index, (name, body) in enumerate(functions):
        path = output / f'{binary.name}-vm-{index}.asm'
        path.write_text(name + ':\n' + '\n'.join(body) + '\n')
        result['vm_functions'].append({'symbol': name, 'file': path.name,
                                      'entry': body[0].split()[0],
                                      'instructions': len(body),
                                      'sp_access_instructions': sum('[sp' in line for line in body),
                                      'frame_instruction': body[7]})
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('binaries', nargs='+', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    result = {binary.name: inspect(binary, args.output) for binary in args.binaries}
    (args.output / 'codegen.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
