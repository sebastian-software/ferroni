#!/usr/bin/env python3
"""Deduplicate a diagnostic JSONL capture without changing call order/identity."""
import argparse
import collections
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('capture', type=Path)
parser.add_argument('output', type=Path)
args = parser.parse_args()
scanners, subjects, calls = [], [], []
scanner_ids, subject_ids = {}, {}
costs = collections.Counter()
for line in args.capture.read_text().splitlines():
    event = json.loads(line)
    sid, tid = event['scanner'], event['subject_id']
    if sid not in scanner_ids:
        scanner_ids[sid] = len(scanners)
        scanners.append(event['patterns'])
    group = scanner_ids[sid]
    assert scanners[group] == event['patterns']
    if tid not in subject_ids:
        subject_ids[tid] = len(subjects)
        subjects.append(event['subject'])
    subject = subject_ids[tid]
    assert subjects[subject] == event['subject']
    expected = event['expected']
    calls.append([group, subject, event['start'], event['options'],
                  None if expected is None else expected['index'],
                  [] if expected is None else expected['captures']])
    costs[group] += event['elapsed_ns']
fixture = dict(format_version=1, scanners=scanners, subjects=subjects, calls=calls,
               hot_groups=[group for group, _ in costs.most_common(6)])
args.output.write_text(json.dumps(fixture, ensure_ascii=False, separators=(',', ':')) + '\n')
summary = dict(calls=len(calls), scanners=len(scanners), subjects=len(subjects),
               diagnostic_group_ns=dict(costs.most_common()),
               trace_sha256=hashlib.sha256(args.output.read_bytes()).hexdigest())
print(json.dumps(summary, indent=2))
