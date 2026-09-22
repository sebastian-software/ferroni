/**
 * Contract check: the numbers on the home page must come from the benchmark
 * page, not from memory.
 *
 * The home page shows a speed factor per benchmark card. This script recomputes
 * every factor from the raw timings in `perf/benchmark-results.mdx` and fails
 * when a card drifts from its source row. Each card is mapped to one explicit
 * section and row, because row labels repeat across sections ("Tokenize full
 * line" exists for both the TypeScript and the Rust grammar). It also verifies
 * that the provenance the home page prints (measurement date and commit)
 * matches the measurement context table on that page.
 *
 * Run with `pnpm check:numbers`; `pnpm build` runs it first.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const here = import.meta.dirname;
const homePath = resolve(here, "../app/routes/home.tsx");
const benchPath = resolve(here, "../app/routes/perf/benchmark-results.mdx");

const home = readFileSync(homePath, "utf8");
const bench = readFileSync(benchPath, "utf8");

/**
 * Home page card label -> the one row it is derived from.
 * `section` is the `###` heading, `group` the bold grammar row inside it
 * (omitted where the table has no groups).
 */
const SOURCE_FOR_CARD = {
  "Scanner First Match": {
    section: "Scanner with full Shiki TextMate grammars",
    group: "TypeScript (279 patterns)",
    row: "First match, short line",
  },
  "Full Line Tokenization": {
    section: "Scanner with full Shiki TextMate grammars",
    group: "TypeScript (279 patterns)",
    row: "Tokenize full line",
  },
  "CSS Tokenization": {
    section: "Scanner with full Shiki TextMate grammars",
    group: "CSS (117 patterns)",
    row: "Tokenize (multi-line)",
  },
  "Rejection Speed": {
    section: "Text search and log scanning",
    row: "No match, 50 KB",
  },
  "RegSet Multi-Pattern": {
    section: "Text search and log scanning",
    row: "RegSet multi-pattern (5)",
  },
  "Lookaround Combined": {
    section: "Pattern matching",
    row: "Lookaround combined",
  },
};

const UNITS = { ns: 1, µs: 1e3, us: 1e3, ms: 1e6, s: 1e9 };
const UNIT_SUFFIXES = Object.keys(UNITS).sort((left, right) => right.length - left.length);

const errors = [];

function toNanoseconds(value) {
  const unit = UNIT_SUFFIXES.find((suffix) => value.endsWith(suffix));
  if (unit === undefined) return null;
  const number = Number(value.slice(0, -unit.length).trim());
  return Number.isFinite(number) ? number * UNITS[unit] : null;
}

function key({ section, group, row }) {
  return [section, group ?? "", row].join(" || ");
}

function groupLabel(cells) {
  if (cells.length < 2 || !cells.slice(1).every((cell) => cell === "")) return null;
  return /^\*\*([^*]+)\*\*$/.exec(cells[0])?.[1] ?? null;
}

function timingRow(cells) {
  if (cells.length < 3) return null;
  const [label, ferroni, oniguruma] = cells.map((cell) => cell.replaceAll("**", ""));
  const ferroniNs = toNanoseconds(ferroni);
  const onigurumaNs = toNanoseconds(oniguruma);
  if (ferroniNs === null || onigurumaNs === null) return null;
  return { label, ferroni: ferroniNs, oniguruma: onigurumaNs };
}

function collectSectionRows({ section, body, rows, duplicates }) {
  let group = null;
  for (const line of body.split("\n")) {
    if (!line.startsWith("|")) continue;
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());
    const nextGroup = groupLabel(cells);
    if (nextGroup !== null) {
      group = nextGroup;
      continue;
    }
    const timing = timingRow(cells);
    if (timing === null) continue;
    const id = key({ section, group, row: timing.label });
    if (rows.has(id)) duplicates.push(id);
    rows.set(id, { ferroni: timing.ferroni, oniguruma: timing.oniguruma });
  }
}

/**
 * Every timing row of the benchmark page, keyed by section, group, and label.
 * A duplicate key means the page itself is ambiguous, which is an error here.
 */
function benchmarkRows() {
  const rows = new Map();
  const duplicates = [];
  const sections = bench.split("\n### ");
  for (const block of sections.slice(1)) {
    const headingEnd = block.indexOf("\n");
    collectSectionRows({
      section: block.slice(0, headingEnd).trim(),
      body: block.slice(headingEnd + 1),
      rows,
      duplicates,
    });
  }

  for (const id of duplicates) {
    errors.push(`perf/benchmark-results.mdx has two rows for "${id}".`);
  }
  return rows;
}

/** The `benchmarks` array literal of the home page. */
function cardField(entry, name) {
  const line = entry.split("\n").find((candidate) => candidate.trimStart().startsWith(`${name}:`));
  if (line === undefined) return null;
  return line
    .slice(line.indexOf(":") + 1)
    .trim()
    .replaceAll(/[",]/g, "");
}

function homeCards() {
  const start = home.indexOf("const benchmarks = [");
  if (start === -1) throw new Error("home.tsx: `benchmarks` array not found");
  const end = home.indexOf("\n]", start);
  return home
    .slice(start, end)
    .split("\n  },")
    .map((entry) => {
      const label = cardField(entry, "label");
      const speedup = cardField(entry, "speedup");
      return label === null || speedup === null
        ? null
        : { label, speedup: Number(speedup.replace("x", "")) };
    })
    .filter((card) => card !== null);
}

const rows = benchmarkRows();
const cards = homeCards();

for (const card of cards) {
  const source = SOURCE_FOR_CARD[card.label];
  if (!source) {
    errors.push(`The "${card.label}" card has no source row. Add it to SOURCE_FOR_CARD.`);
    continue;
  }
  const measured = rows.get(key(source));
  if (!measured) {
    errors.push(
      `"${key(source)}" is not a row of perf/benchmark-results.mdx ` +
        `(mapped from the "${card.label}" card).`,
    );
    continue;
  }
  const factor = Math.round((measured.oniguruma / measured.ferroni) * 10) / 10;
  if (Math.abs(factor - card.speedup) > 0.05) {
    errors.push(
      `"${card.label}" claims ${card.speedup}x, but "${key(source)}" measures ${factor}x.`,
    );
  }
}

for (const label of Object.keys(SOURCE_FOR_CARD)) {
  if (!cards.some((card) => card.label === label)) {
    errors.push(`SOURCE_FOR_CARD maps "${label}", which is no longer a card on the home page.`);
  }
}

const context = {
  commit: bench.match(/Ferroni commit\s*\| `([0-9a-f]+)`/)?.[1],
  date: bench.match(/Measurement date\s*\| `([\d-]+)/)?.[1],
};
if (!context.commit || !context.date) {
  errors.push("perf/benchmark-results.mdx: measurement context table not found.");
} else {
  if (!home.includes(context.date)) {
    errors.push(`home.tsx does not name the measurement date ${context.date}.`);
  }
  if (!home.includes(context.commit.slice(0, 8))) {
    errors.push(`home.tsx does not name the measurement commit ${context.commit.slice(0, 8)}.`);
  }
}

if (errors.length > 0) {
  console.error("Benchmark claims on the home page do not match their source:\n");
  for (const error of errors) console.error(`  - ${error}`);
  console.error("\nUpdate app/routes/home.tsx and app/routes/perf/benchmark-results.mdx together.");
  process.exit(1);
}

console.log(`Benchmark claims check: ${cards.length} cards match their source rows.`);
