/**
 * Contract check: the numbers on the home page must come from the benchmark
 * page, not from memory.
 *
 * The home page shows a speed factor per benchmark card. This script recomputes
 * every factor from the raw timings in `perf/benchmark-results.mdx` and fails
 * when a card drifts from its source row. It also verifies that the provenance
 * the home page prints (measurement date and commit) matches the measurement
 * context table on that page.
 *
 * Run with `pnpm check:numbers`; `pnpm build` runs it first.
 */

import { readFileSync } from "node:fs"
import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"

const here = dirname(fileURLToPath(import.meta.url))
const homePath = resolve(here, "../app/routes/home.tsx")
const benchPath = resolve(here, "../app/routes/perf/benchmark-results.mdx")

const home = readFileSync(homePath, "utf8")
const bench = readFileSync(benchPath, "utf8")

/** Home page card label -> row label in benchmark-results.mdx. */
const ROW_FOR_CARD = {
  "Scanner First Match": "First match, short line",
  "Full Line Tokenization": "Tokenize full line",
  "CSS Tokenization": "Tokenize (multi-line)",
  "Rejection Speed": "No match, 50 KB",
  "RegSet Multi-Pattern": "RegSet multi-pattern (5)",
  "Lookaround Combined": "Lookaround combined",
}

const UNITS = { ns: 1, "µs": 1e3, us: 1e3, ms: 1e6, s: 1e9 }

const errors = []

function toNanoseconds(value) {
  const match = value.match(/([\d.]+)\s*(ns|µs|us|ms|s)/)
  if (!match) return null
  return Number(match[1]) * UNITS[match[2]]
}

/** Every `| label | ferroni | oniguruma | ... |` row of the benchmark page. */
function benchmarkRows() {
  const rows = new Map()
  for (const line of bench.split("\n")) {
    if (!line.startsWith("|")) continue
    const cells = line
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim().replace(/\*\*/g, ""))
    if (cells.length < 3) continue
    const [label, ferroni, oniguruma] = cells
    const a = toNanoseconds(ferroni)
    const b = toNanoseconds(oniguruma)
    if (a === null || b === null) continue
    // A label may appear once per grammar section; keep every occurrence.
    const list = rows.get(label) ?? []
    list.push({ ferroni: a, oniguruma: b })
    rows.set(label, list)
  }
  return rows
}

/** The `benchmarks` array literal of the home page. */
function homeCards() {
  const start = home.indexOf("const benchmarks = [")
  if (start === -1) throw new Error("home.tsx: `benchmarks` array not found")
  const end = home.indexOf("\n]", start)
  const block = home.slice(start, end)
  const cards = []
  for (const entry of block.split(/\{\s*\n/).slice(1)) {
    const field = (name) =>
      entry.match(new RegExp(`${name}:\\s*"([^"]*)"`))?.[1] ?? null
    const label = field("label")
    const speedup = field("speedup")
    if (label && speedup) cards.push({ label, speedup: Number(speedup.replace("x", "")) })
  }
  return cards
}

const rows = benchmarkRows()
const cards = homeCards()

if (cards.length !== Object.keys(ROW_FOR_CARD).length) {
  errors.push(
    `home.tsx has ${cards.length} benchmark cards, the mapping in this script covers ` +
      `${Object.keys(ROW_FOR_CARD).length}. Add the new card to ROW_FOR_CARD.`,
  )
}

for (const card of cards) {
  const rowLabel = ROW_FOR_CARD[card.label]
  if (!rowLabel) {
    errors.push(`No source row mapped for the "${card.label}" card.`)
    continue
  }
  const candidates = rows.get(rowLabel)
  if (!candidates) {
    errors.push(`"${rowLabel}" is not a row of perf/benchmark-results.mdx.`)
    continue
  }
  // Pick the row whose factor is closest; several grammars share a label.
  const factors = candidates.map((row) => row.oniguruma / row.ferroni)
  const best = factors.reduce((a, b) =>
    Math.abs(b - card.speedup) < Math.abs(a - card.speedup) ? b : a,
  )
  const rounded = Math.round(best * 10) / 10
  if (Math.abs(rounded - card.speedup) > 0.05) {
    errors.push(
      `"${card.label}" claims ${card.speedup}x, but "${rowLabel}" measures ${rounded}x.`,
    )
  }
}

const context = {
  commit: bench.match(/Ferroni commit \| `([0-9a-f]+)`/)?.[1],
  date: bench.match(/Measurement date \| `([\d-]+)/)?.[1],
}
if (!context.commit || !context.date) {
  errors.push("perf/benchmark-results.mdx: measurement context table not found.")
} else {
  if (!home.includes(context.date)) {
    errors.push(`home.tsx does not name the measurement date ${context.date}.`)
  }
  if (!home.includes(context.commit.slice(0, 8))) {
    errors.push(
      `home.tsx does not name the measurement commit ${context.commit.slice(0, 8)}.`,
    )
  }
}

if (errors.length > 0) {
  console.error("Benchmark claims on the home page do not match their source:\n")
  for (const error of errors) console.error(`  - ${error}`)
  console.error(
    "\nUpdate app/routes/home.tsx and app/routes/perf/benchmark-results.mdx together.",
  )
  process.exit(1)
}

console.log(`Benchmark claims check: ${cards.length} cards match their source rows.`)
