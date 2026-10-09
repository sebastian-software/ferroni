import {
  type ComparisonCell,
  type ComparisonRow,
  ComparisonTable,
  EvidenceFigures,
  family,
  Mark,
  Measured,
  Principles,
  ProjectHero,
  Relations,
  RunSample,
  Section,
  WorkWithUs,
} from "ferramenta-family";
import { Link, type MetaFunction } from "react-router";
import config from "virtual:ardo/config";

import engineComparison from "../data/engine-comparison.json";
import sample from "../data/regex-sample.json";
import {
  ClosingSection,
  CoverageSection,
  ProofSection,
  SafetySection,
  ScannerSection,
} from "./home-bottom-sections";

// React Router requires `meta` as a named route export.
// oxlint-disable-next-line react/only-export-components -- React Router requires this route export.
export const meta: MetaFunction = () => [
  { title: "Ferroni — Oniguruma's regex engine, modernized in Rust" },
  {
    name: "description",
    content:
      "Full Oniguruma syntax for Rust, with the vscode-oniguruma scanner that TextMate grammars run on. One cargo add, no C compiler, memory-safe, verified against the upstream tests, and measured against the C original.",
  },
];

function ferroni() {
  const tool = family.find((entry) => entry.name === "ferroni");
  if (tool === undefined) throw new Error("Ferroni is missing from the family registry.");
  return tool;
}

type FigureCell = {
  text: string;
  note?: string;
  grammars?: string[];
  cases: number;
  of: number;
  /** Per host; the summary shows only `factor`, their geometric mean. */
  factors?: Record<string, number>;
  factor?: number;
  /** "faster" or "slower": which way `text` reads. */
  direction?: string;
};

/* The cell covers only the cases the engine could run: footnoted, not spelled out. */
const partial = (cell: FigureCell) => cell.factor !== undefined && cell.cases < cell.of;

const workloads = engineComparison.workloads;

/* The C original's row: the figures the speed claims on this page rest on. */
const cRow = engineComparison.engines.find((engine) => engine.id === "c");
if (cRow === undefined) throw new Error("engine-comparison.json has no row for Oniguruma (C).");
const cCells = cRow.cells as Record<string, FigureCell>;

/*
 * "Faster than the original" is only said while it holds: on every host, in
 * every workload of the retained run. A run where C wins a workload turns the
 * headline into a plain "measured against".
 */
const cFactors = workloads.flatMap((workload) => Object.values(cCells[workload.id].factors ?? {}));
const aheadOfC = cFactors.length > 0 && cFactors.every((factor) => factor > 1);

/* The narrowest and widest lead over C, as the table states them: "1.4× to 12×". */
const cTexts = workloads
  .map((workload) => cCells[workload.id])
  .sort((a, b) => (a.factor ?? 0) - (b.factor ?? 0))
  .map((cell) => cell.text);
const cRange = `${cTexts[0]} to ${cTexts.at(-1)}`;

/*
 * What it succeeds and what it is checked against come from the registry; the
 * release is the version this site was built from, read from Cargo.toml.
 */
function HeroSection() {
  const tool = ferroni();
  const version = config.project?.version;
  return (
    <ProjectHero
      icon="ferroni"
      title={<span translate="no">Ferroni</span>}
      what="Oniguruma's regex engine, modernized in Rust."
      lede={
        <>
          Look-behind, backreferences, Unicode properties and the multi-pattern scanner that
          TextMate grammars run on, in one pure-Rust crate. No C compiler, memory-safe
          {aheadOfC
            ? ", and faster than the C original."
            : ", and measured against the C original."}
        </>
      }
      actions={
        <>
          <Link to="/guide/getting-started" className="fam-btn fam-btn-primary">
            Get started <Mark name="arrow" className="icon" size={18} />
          </Link>
          <a href="https://github.com/sebastian-software/ferroni" className="fam-btn fam-btn-ghost">
            <Mark name="github" className="icon" size={18} /> GitHub
          </a>
        </>
      }
      install={<code translate="no">cargo add ferroni</code>}
      facts={[
        { label: "Succeeds", value: tool.succeeds },
        { label: "Checked against", value: tool.evidence },
        ...(version == null ? [] : [{ label: "Release", value: `v${version}` }]),
      ]}
    />
  );
}

const pillars = [
  {
    heading: "Same engine, verified",
    text: "A line-by-line port of Oniguruma's parser, compiler and optimizer. All 2,974 upstream UTF-8 test cases pass, so a pattern behaves as it does in C.",
  },
  {
    heading: "Easy to add",
    text: (
      <>
        <code>cargo add ferroni</code> and build: no C compiler, no bindgen, four common
        dependencies. An idiomatic <code>Regex</code> API on top, <code>Send + Sync</code> for
        sharing across threads.
      </>
    ),
  },
  {
    heading: "Safe with untrusted input",
    text: "Memory-safe Rust, with unsafe confined to the links between parse-tree nodes. Timeouts and retry limits per search, a compile-time backtracking check, and continuous fuzzing.",
  },
  aheadOfC
    ? {
        heading: "Faster than the original",
        text: `Ahead of C Oniguruma in every measured workload: ${cRange} faster, from everyday patterns to syntax highlighting, with inputs and raw data published.`,
      }
    : {
        heading: "Measured against C",
        text: "Every workload is timed against C Oniguruma on two hosts, with the inputs, versions and raw data published.",
      },
];

function ForwardSection() {
  return (
    <Section
      id="fr-forward"
      title="Same engine. Modern Rust."
      intro="Oniguruma’s C project closed on April 24, 2025, after more than twenty years as the engine TextMate grammars are written for. Ferroni carries it forward, and adds what C Oniguruma never shipped: Unicode 18.0, the vscode-oniguruma scanner, a backtracking check and an idiomatic Rust API."
    >
      <Principles items={pillars} />
    </Section>
  );
}

function SampleSection() {
  return (
    <Section
      id="fr-run"
      title="Familiar Rust, full Oniguruma syntax"
      intro={
        <>
          The look-behind that selects this date is syntax Rust&rsquo;s <code>regex</code> crate
          does not run; the named groups and the API around them are the ones you already know. The
          output is what the example printed, committed with the site.
        </>
      }
      note={
        <a href="https://github.com/sebastian-software/ferroni/blob/main/examples/website_sample.rs">
          Run the example: cargo run --example website_sample
        </a>
      }
    >
      <RunSample
        input={sample.input}
        inputCaption="website_sample.rs"
        inputKind="Rust source"
        output={sample.output}
        outputCaption={`Ferroni ${sample.version} · stdout`}
      />
      <details className="fr-sample-output">
        <summary>Read the output as text</summary>
        <pre tabIndex={0}>
          <code>{sample.stdout}</code>
        </pre>
      </details>
    </Section>
  );
}

function figureValue(cell: FigureCell): ComparisonCell {
  if (cell.factor === undefined) {
    return cell.grammars === undefined ? undefined : { mark: "no", note: cell.grammars.join(", ") };
  }
  return (
    <>
      {cell.text} <small>{cell.direction}</small>
      {partial(cell) && (
        <>
          <sup aria-hidden="true">*</sup>
          <span className="fam-sr-only">, some tasks left out</span>
        </>
      )}
    </>
  );
}

/* Every figure comes from docs/app/data/engine-comparison.json, which
 * `scripts/compare-engines.py figures` derives from the retained run. */
const comparisonRows: ComparisonRow[] = engineComparison.engines.map((engine) => {
  const cells = engine.cells as Record<string, FigureCell>;
  const measured = workloads.flatMap((workload) => Object.values(cells[workload.id].factors ?? {}));
  return {
    label: engine.label,
    values: Object.fromEntries(
      workloads.map((workload) => [workload.id, figureValue(cells[workload.id])]),
    ),
    // Ferroni is slower in every workload this engine was measured in.
    behind: measured.length > 0 && measured.every((factor) => factor < 1),
  };
});

/* The C original's factor per workload, stamped on plates above the full table. */
const cFigures = workloads.map((workload) => {
  const cell = cCells[workload.id];
  return {
    label: workload.label,
    value: cell.direction === "slower" ? `${cell.text} slower` : cell.text,
    detail: workload.detail,
  };
});

/* The footnotes the table needs, worded once in compare-engines.py. */
const allCells = engineComparison.engines.flatMap((engine) =>
  workloads.map((workload) => (engine.cells as Record<string, FigureCell>)[workload.id]),
);
const tableNotes = [
  ...(allCells.some((cell) => partial(cell)) ? [`* ${engineComparison.notes.partial}`] : []),
  ...(allCells.some((cell) => cell.text === "–") ? [`– ${engineComparison.notes.absent}`] : []),
  // Which of fancy-regex's configurations each column quotes.
  engineComparison.notes.configurations,
];

function SpeedSection() {
  const hosts = engineComparison.hosts
    .map((host) => `${host.label}: ${host.machine}, ${host.cpus} vCPUs`)
    .join("; ");
  return (
    <Section
      id="fr-evidence"
      layout="split"
      title={aheadOfC ? "Faster than the C original" : "Measured against the C original"}
      intro={
        <>
          How much faster Ferroni is than C Oniguruma on three kinds of work, as the geometric mean
          over every task and two test machines. The table adds six more engines. Each one first has
          to reproduce Oniguruma&rsquo;s results, or the results Shiki produced for highlighting,
          before it is timed.
        </>
      }
      note={
        <>
          Where Ferroni is behind, the table says so. When patterns fit their syntax, Rust&rsquo;s{" "}
          <code>regex</code> crate and PCRE2&rsquo;s JIT win most text tasks; neither finds the
          earliest match among many patterns in one search, as a highlighter needs. C Oniguruma wins
          a few individual tasks, and grammar compilation is mixed.
        </>
      }
    >
      <EvidenceFigures figures={cFigures} />
      <ComparisonTable
        align="end"
        caption="How much faster or slower Ferroni is than each engine."
        subject="Compared with"
        contenders={workloads.map((workload) => ({ id: workload.id, label: workload.short }))}
        rows={comparisonRows}
      />
      {tableNotes.map((note) => (
        <p key={note} className="fam-note">
          {note}
        </p>
      ))}
      <Measured
        on={engineComparison.measured}
        machine={`Blacksmith runners. ${hosts}`}
        revision={<code>{engineComparison.commit.slice(0, 8)}</code>}
      >
        <Link to="/perf/engine-comparison">Per-host figures, every task and the raw data</Link>
      </Measured>
    </Section>
  );
}

function RelationsSection() {
  return (
    <Section
      id="fr-relations"
      title="Where Ferroni sits"
      intro="Ferroni supplies the regex engine for Ferriki, whose Shiki-compatible highlighter tokenizes code with TextMate grammars. Each tool also works on its own."
    >
      <Relations current="ferroni" />
    </Section>
  );
}

export default function HomePage() {
  return (
    <div className="fam-page ferroni-home">
      <HeroSection />
      <ForwardSection />
      <SampleSection />
      <SpeedSection />
      <ScannerSection />
      <SafetySection />
      <ProofSection />
      <CoverageSection />
      <RelationsSection />
      <ClosingSection />
      <WorkWithUs />
    </div>
  );
}
