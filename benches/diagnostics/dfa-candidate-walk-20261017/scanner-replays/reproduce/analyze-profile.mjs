// Summarizes a samply profile recorded with --unstable-presymbolicate (#225):
// self and inclusive shares per symbol on the busiest thread, optionally the
// subtree below one symbol or the callers of a leaf.
import { readFileSync } from "node:fs";
import process from "node:process";
import { parseArgs } from "node:util";
import { gunzipSync } from "node:zlib";

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    top: { type: "string", default: "25" },
    since: { type: "string" },
    focus: { type: "string" },
    callers: { type: "string" },
    help: { type: "boolean" },
  },
});
if (values.help || positionals.length !== 1) {
  console.log(
    "Usage: node scripts/summarize-samply-profile.mjs profile.json.gz [--since <epoch ms>] [--top 25] [--focus <symbol regex>] [--callers <leaf regex>]",
  );
  process.exit(values.help ? 0 : 1);
}
const [file] = positionals;
const top = Number(values.top);
const read = (path) => readFileSync(path);
const profile = JSON.parse(
  file.endsWith(".gz") ? gunzipSync(read(file)) : read(file).toString("utf8"),
);
const symbols = JSON.parse(read(`${file.replace(/\.gz$/, "")}.syms.json`));
const tables = new Map(symbols.data.map((entry) => [entry.code_id, entry.symbol_table]));
const thread = profile.threads.toSorted((a, b) => b.samples.length - a.samples.length)[0];

const names = new Map();
function frameName(frame) {
  if (names.has(frame)) return names.get(frame);
  const func = thread.frameTable.func[frame];
  const resource = thread.funcTable.resource[func];
  let name;
  if (resource == null || resource < 0) name = thread.stringArray[thread.funcTable.name[func]];
  else {
    const library = profile.libs[thread.resourceTable.lib[resource]];
    const table = tables.get(library.codeId) ?? [];
    const address = thread.frameTable.address[frame];
    let low = 0;
    let high = table.length - 1;
    let hit;
    while (low <= high) {
      const middle = (low + high) >> 1;
      if (table[middle].rva <= address) {
        hit = table[middle];
        low = middle + 1;
      } else high = middle - 1;
    }
    const symbol = hit ? symbols.string_table[hit.symbol] : `0x${address.toString(16)}`;
    name = `${library.name}!${symbol}`.replace(/::h[0-9a-f]{16}$/, "");
  }
  names.set(frame, name);
  return name;
}

// Stacks are leaf first. `since` drops warm-up samples by wall-clock time.
const stacks = [];
let time = profile.meta.startTime;
for (let i = 0; i < thread.samples.length; i++) {
  time += thread.samples.timeDeltas[i];
  let stack = thread.samples.stack[i];
  if (stack == null || (values.since && (time < Number(values.since) || time >= Number(values.since) + 19000))) continue;
  const chain = [];
  while (stack != null) {
    chain.push(frameName(thread.stackTable.frame[stack]));
    stack = thread.stackTable.prefix[stack];
  }
  stacks.push(chain);
}
const percent = (count, total) => `${((100 * count) / total).toFixed(2).padStart(6)}%`;
const short = (name) => name.slice(0, 160);
const print = (title, counts, total) => {
  console.log(`\n== ${title} ==`);
  for (const [name, count] of [...counts].sort((a, b) => b[1] - a[1]).slice(0, top))
    console.log(`${percent(count, total)}  ${name}`);
};
const add = (counts, name) => counts.set(name, (counts.get(name) ?? 0) + 1);

console.log(`${stacks.length} samples`);
const self = new Map();
const inclusive = new Map();
for (const chain of stacks) {
  add(self, short(chain[0]));
  for (const name of new Set(chain.map(short))) add(inclusive, name);
}
print("self", self, stacks.length);
print("inclusive", inclusive, stacks.length);

if (values.focus) {
  const pattern = new RegExp(values.focus);
  const leaves = new Map();
  const children = new Map();
  let count = 0;
  for (const chain of stacks) {
    const index = chain.findLastIndex((name) => pattern.test(name));
    if (index < 0) continue;
    count++;
    add(leaves, short(chain[0]));
    add(children, index > 0 ? short(chain[index - 1]) : "(self)");
  }
  console.log(
    `\n${count} samples (${percent(count, stacks.length).trim()}) under /${values.focus}/`,
  );
  print("leaf symbols in subtree", leaves, count);
  print("direct children", children, count);
}

if (values.callers) {
  const pattern = new RegExp(values.callers);
  const chains = new Map();
  for (const chain of stacks)
    if (pattern.test(chain[0]))
      add(
        chains,
        chain
          .slice(0, 4)
          .map((name) => name.slice(0, 120))
          .join("\n          <- "),
      );
  print(`callers of /${values.callers}/`, chains, stacks.length);
}

const { writeFileSync } = await import('node:fs');
const patterns = {
  ferroni: /ferroni::|regex_automata::|aho_corasick::/,
  dfaCandidates: /SetPrefilter>::candidates_at/,
  injectionMatcher: /ferriki_textmate::matcher::Matcher>::matches/,
  scopePush: /AttributedScopeStack>::push_attributed/,
  scopeState: /AttributedScopeStack|StateStack|ScopeStack/,
  inlineRender: /ferriki::render::/,
  themeMatch: /ferriki_textmate::theme::Theme>::match_scope/,
  themeActivation: /HighlighterCore>::activate_theme|SyncRegistry>::set_theme/,
  colorMap: /get_color_map/,
  onigString: /OnigString>::new/,
  napiConvert: /ToNapiValue|napi::bindgen_runtime.*to_napi_value/,
  allocationLeaf: /libsystem_malloc|RawVecInner>::finish_grow|__rust_alloc|__rust_realloc/,
};
const groups = Object.fromEntries(Object.entries(patterns).map(([label,pattern]) => {
  const included = stacks.filter(chain => chain.some(name => pattern.test(name)));
  const leaves = new Map();
  for (const chain of included) add(leaves, chain[0]);
  return [label,{samples:included.length,percent:100*included.length/stacks.length,topLeaves:[...leaves].sort((a,b)=>b[1]-a[1]).slice(0,8)}];
}));
writeFileSync(file+'.analysis.json',JSON.stringify({samples:stacks.length,groups,self:[...self].sort((a,b)=>b[1]-a[1]),inclusive:[...inclusive].sort((a,b)=>b[1]-a[1])},null,2)+'\n');
console.log(JSON.stringify({samples:stacks.length,groups:Object.fromEntries(Object.entries(groups).map(([k,v])=>[k,+v.percent.toFixed(2)]))}));
