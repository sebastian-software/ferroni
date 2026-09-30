"""Summarize two macOS samples; inclusive categories overlap and cannot be added."""
import collections
import json
import gzip
import sys
import pathlib
import re

paths = [('document', sys.argv[1]), ('group_78', sys.argv[2])]
result = {}
for api, path in paths:
    data = pathlib.Path(path).read_bytes()
    text = (gzip.decompress(data) if path.endswith('.gz') else data).decode()
    graph = text.split('Call graph:\n', 1)[1].split('\nTotal number in stack', 1)[0]
    lines = graph.splitlines()
    total = int(re.search(r'(\d+) Thread_', lines[0]).group(1))
    nodes = []
    stack = []
    for line in lines[1:]:
        if re.match(r'^    \d+ Thread_', line):
            break
        match = re.match(r'^([ +!|:]*)?(\d+) (.+)$', line)
        if not match:
            continue
        indent, count, function = len(match.group(1)), int(match.group(2)), match.group(3)
        while stack and stack[-1]['indent'] >= indent:
            stack.pop()
        node = {'indent': indent, 'count': count, 'function': function, 'parent': stack[-1] if stack else None, 'children': []}
        if node['parent']:
            node['parent']['children'].append(node)
        nodes.append(node)
        stack.append(node)
    for node in nodes:
        assert sum(n['count'] for n in node['children']) <= node['count'], node['function']
    categories = {
        'ferroni': ['ferroni::'],
        'scanner': ['ferroni::scanner::Scanner::find_next_match_utf16'],
        'regset': ['ferroni::regset::'],
        'match_at': ['ferroni::regexec::match_at_impl'],
        'fallback_search': ['ferroni::regset::search_fallback_entry'],
        'string_setup': ['ferroni::scanner::OnigString::new'],
    }
    shares = {}
    for category, patterns in categories.items():
        count = 0
        for node in nodes:
            if not any(p in node['function'] for p in patterns):
                continue
            parent = node['parent']
            while parent and not any(p in parent['function'] for p in patterns):
                parent = parent['parent']
            if parent is None:
                count += node['count']
        shares[category] = {'inclusiveSamples': count, 'mainThreadPercent': count / total * 100}
    exclusive = collections.Counter()
    for node in nodes:
        count = node['count'] - sum(n['count'] for n in node['children'])
        if 'ferroni::' in node['function']:
            exclusive[re.sub(r'::h[0-9a-f]+.*', '', node['function'])] += count
    result[api] = {'mainThreadSamples': total, 'inclusive': shares, 'ferroniExclusiveTop': exclusive.most_common(15)}
print(json.dumps(result, indent=2))
