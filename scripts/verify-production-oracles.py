#!/usr/bin/env python3
"""Replay original patterns in Node, Python, Ruby, and the PCRE2 C API.

This verifies isolated regex steps, not complete applications. Requires Node
with match indices, Ruby, and libpcre2-8 on the host. No network access or
upstream package installation is needed. Runtime versions are printed.
"""

import ctypes
import ctypes.util
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "benches/production_regex/catalog.json"

NODE = r"""
const fs = require('fs');
const cases = JSON.parse(fs.readFileSync(0, 'utf8'));
const results = cases.map(c => c.inputs.map(i => {
  const re = new RegExp(c.original_pattern, 'd' + (c.flags.ignore_case ? 'i' : ''));
  const m = re.exec(i.text);
  if (c.operation === 'is_match') return m !== null;
  return m ? m.indices.map(b => b ? b.map(n => Buffer.byteLength(i.text.slice(0,n))) : null) : null;
}));
process.stdout.write(JSON.stringify(results));
"""
RUBY = r"""
require 'json'
cases = JSON.parse(STDIN.read)
results = cases.map do |c|
  options = 0
  options |= Regexp::IGNORECASE if c['flags']['ignore_case']
  options |= Regexp::MULTILINE if c['flags']['dotall']
  options |= Regexp::EXTENDED if c['flags']['extended']
  regex = Regexp.new(c['original_pattern'], options)
  c['inputs'].map do |i|
    text = i['text']
    position = text.byteslice(0, i['start']).length
    if c['operation'] == 'replace'
      text.gsub(regex) { c['replacement'] }
    elsif c['operation'] == 'all_captures'
      groups = []
      text.scan(regex) do
        m = Regexp.last_match
        groups << (0...m.length).map { |n| m.begin(n).nil? ? nil : [text[0...m.begin(n)].bytesize, text[0...m.end(n)].bytesize] }
      end
      groups
    else
      m = regex.match(text, position)
      m && (0...m.length).map { |n| m.begin(n).nil? ? nil : [text[0...m.begin(n)].bytesize, text[0...m.end(n)].bytesize] }
    end
  end
end
STDOUT.write(JSON.generate(results))
"""


def python_outputs(case):
    flags = sum(flag for name, flag in (("ignore_case", re.I), ("dotall", re.S), ("extended", re.X), ("multiline", re.M)) if case["flags"].get(name))
    regex = re.compile(case["original_pattern"], flags)
    results = []
    for fixture in case["inputs"]:
        text = fixture["text"]
        start = len(text.encode()[:fixture["start"]].decode())
        match = regex.match(text, start) if case["operation"] == "captures_at" else regex.search(text, start)
        if case["operation"] == "is_match":
            results.append(match is not None)
        elif match is None:
            results.append(None)
        else:
            results.append([None if match.start(n) < 0 else [len(text[:match.start(n)].encode()), len(text[:match.end(n)].encode())] for n in range(regex.groups + 1)])
    return results


def pcre_outputs(case, library):
    compile = library.pcre2_compile_8
    compile.restype = ctypes.c_void_p
    compile.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint32, ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_size_t), ctypes.c_void_p]
    data_from_pattern = library.pcre2_match_data_create_from_pattern_8
    data_from_pattern.restype = ctypes.c_void_p
    data_from_pattern.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    match = library.pcre2_match_8
    match.restype = ctypes.c_int
    match.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_size_t, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_void_p]
    vector = library.pcre2_get_ovector_pointer_8
    vector.restype = ctypes.POINTER(ctypes.c_size_t)
    vector.argtypes = [ctypes.c_void_p]
    count = library.pcre2_get_ovector_count_8
    count.restype = ctypes.c_uint32
    count.argtypes = [ctypes.c_void_p]
    for free in (library.pcre2_code_free_8, library.pcre2_match_data_free_8):
        free.argtypes = [ctypes.c_void_p]
    # Source WordPress /s has ASCII \w and no /u. Symfony chunk uses /u.
    flags = 0x20 if case["flags"].get("dotall") else 0  # PCRE2_DOTALL
    if case["operation"] == "graphemes":
        flags |= 0x80000  # PCRE2_UTF
    error, at = ctypes.c_int(), ctypes.c_size_t()
    pattern = case["original_pattern"].encode()
    code = compile(pattern, len(pattern), flags, ctypes.byref(error), ctypes.byref(at), None)
    if not code:
        raise RuntimeError(f"PCRE2 source compile error {error.value} at {at.value}")
    data = data_from_pattern(code, None)
    try:
        outputs = []
        for fixture in case["inputs"]:
            text = fixture["text"].encode()
            matches, start = [], 0
            while start <= len(text):
                result = match(code, text, len(text), start, 0, data, None)
                if result == -1:
                    break
                if result < -1:
                    raise RuntimeError(f"PCRE2 source execution error {result}")
                v = vector(data)
                groups = [None if v[2*n] == ctypes.c_size_t(-1).value else [v[2*n], v[2*n+1]] for n in range(count(data))]
                matches.append(groups)
                assert groups[0][1] > groups[0][0], "these source patterns must consume text"
                start = groups[0][1]
            if case["operation"] == "graphemes":
                chunks, previous = [], 0
                for groups in matches:
                    begin, end = groups[0]
                    if previous < begin:
                        chunks.append(text[previous:begin].decode())
                    chunks.append(text[begin:end].decode())
                    previous = end
                if previous < len(text):
                    chunks.append(text[previous:].decode())
                outputs.append(chunks)
            else:
                outputs.append(matches)
        return outputs
    finally:
        library.pcre2_match_data_free_8(data)
        library.pcre2_code_free_8(code)


def main():
    cases = [c for c in json.loads(CATALOG.read_text())["cases"] if not c.get("pending_reason")]
    node_cases = [c for c in cases if c.get("source_runtime") == "ECMAScript RegExp"]
    ruby_cases = [c for c in cases if c["id"].startswith(("fluent_bit", "rails", "mastodon"))]
    pcre_cases = [c for c in cases if c["id"].startswith(("wordpress", "symfony"))]
    python_cases = [c for c in cases if c not in node_cases + ruby_cases + pcre_cases]
    observations = []
    for command, group in ((["node", "-e", NODE], node_cases), (["ruby", "-e", RUBY], ruby_cases)):
        result = subprocess.run(command, input=json.dumps(group), text=True, capture_output=True, check=True)
        observations.extend(zip(group, json.loads(result.stdout)))
    path = ctypes.util.find_library("pcre2-8")
    if not path:
        raise RuntimeError("libpcre2-8 is required for the original PHP regex-step oracles")
    library = ctypes.CDLL(path)
    version = ctypes.create_string_buffer(128)
    library.pcre2_config_8.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
    library.pcre2_config_8(11, version)  # PCRE2_CONFIG_VERSION
    observations.extend((c, pcre_outputs(c, library)) for c in pcre_cases)
    for c in python_cases:
        if c["pattern_origin"] == "controlled":
            outputs = [None if i["text"].find(c["original_pattern"]) < 0 else [i["text"].find(c["original_pattern"]), i["text"].find(c["original_pattern"]) + len(c["original_pattern"])] for i in c["inputs"]]
        else:
            outputs = python_outputs(c)
        observations.append((c, outputs))
    for case, actual in observations:
        expected = [i["expected"] for i in case["inputs"]]
        if actual != expected:
            for index, (got, wanted) in enumerate(zip(actual, expected)):
                if got != wanted:
                    raise AssertionError(f"{case['id']} input {index}: expected {wanted!r}, got {got!r}")
        print(f"{case['id']}: {len(actual)} original-step oracle fixtures passed")
    print(subprocess.check_output(["node", "--version"], text=True).strip())
    print(subprocess.check_output(["ruby", "--version"], text=True).strip())
    print(f"Python {sys.version}; PCRE2 {version.value.decode()} at {path}")


if __name__ == "__main__":
    main()
