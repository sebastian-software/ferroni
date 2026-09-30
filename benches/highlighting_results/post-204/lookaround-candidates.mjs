// Static diagnosis only: record literal lists that Ferroni excludes in anchors.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const [ferrikiPath, tracePath, outputPath] = process.argv.slice(2);
assert.ok(outputPath, 'Usage: node lookaround-candidates.mjs FERRIKI_ROOT CPP_TRACE.json OUTPUT.json');
const { parse } = await import(pathToFileURL(join(resolve(ferrikiPath), 'node/node_modules/.pnpm/oniguruma-parser@0.12.1/node_modules/oniguruma-parser/dist/parser/parse.js')));
const bytes=readFileSync(tracePath), fixture=JSON.parse(bytes);
const sha = value => createHash('sha256').update(value).digest('hex');
const patterns=[];
for (const index of [19,100,101,102,118]) {
  const source=fixture.scanners[78][index];
  const ast=parse(source,{rules:{allowOrphanBackrefs:true,captureGroup:true}});
  const lists=[];
  function visit(node, context=[]) {
    if (!node || typeof node !== 'object') return;
    if (node.type === 'LookaroundAssertion') context=[...context,{kind:node.kind,negate:node.negate}];
    if (context.length && node.body?.length >= 4 && node.body.every(a => a.type === 'Alternative' && a.body.length && a.body.every(c => c.type === 'Character' && c.value < 128))) {
      const words=node.body.map(a => a.body.map(c => String.fromCodePoint(c.value)).join(''));
      lists.push({nodeType:node.type,context,alternatives:words.length,uniqueAlternatives:new Set(words).size,words});
    }
    for (const [key,value] of Object.entries(node)) {
      if (key === 'parent') continue;
      if (Array.isArray(value)) value.forEach(v => visit(v,context));
      else if (value && typeof value === 'object') visit(value,context);
    }
  }
  visit(ast);
  patterns.push({index,sourceSha256:sha(source),literalListsInLookarounds:lists});
}
writeFileSync(outputPath,JSON.stringify({parserVersion:'0.12.1',fixtureSha256:sha(bytes),group:78,boundary:'Static original-pattern diagnosis; no rewrite or safety/performance proof',patterns},null,2)+'\n');
console.log(JSON.stringify(patterns.map(p => ({index:p.index,lists:p.literalListsInLookarounds.map(l => ({context:l.context,alternatives:l.alternatives,unique:l.uniqueAlternatives}))}))));
